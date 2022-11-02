use crate::SignatureSource;
use ethabi::{Event, EventParam, ParamType, RawLog, Token};
use itertools::Itertools;
use sig_provider_proto::blockscout::sig_provider::v1::{Abi, Argument};
use std::{collections::HashSet, sync::Arc};

pub struct SourceAggregator {
    sources: Vec<Arc<dyn SignatureSource + Send + Sync + 'static>>,
}

macro_rules! proxy {
    ($sources:ident, $request:ident, $fn:ident) => {{
        let tasks = $sources.iter().map(|source| source.$fn($request));
        let responses: Vec<_> = futures::future::join_all(tasks)
            .await
            .into_iter()
            .zip($sources.iter())
            .filter_map(|(resp, source)| match resp {
                Ok(resp) => Some(resp),
                Err(error) => {
                    tracing::error!(
                        "could not call {} for host {}, error: {}",
                        stringify!($fn),
                        source.source(),
                        error
                    );
                    None
                }
            })
            .collect();
        responses
    }};
}

impl SourceAggregator {
    // You should provide sources in priority descending order (first - max priority)
    pub fn new(sources: Vec<Arc<dyn SignatureSource + Send + Sync + 'static>>) -> SourceAggregator {
        SourceAggregator { sources }
    }

    fn merge_signatures<I: IntoIterator<Item = String>, II: IntoIterator<Item = I>>(
        sigs: II,
    ) -> Vec<String> {
        let mut content: HashSet<String> = HashSet::default();
        sigs.into_iter()
            .flatten()
            .filter(|sig| content.insert(sig.clone()))
            .collect()
    }

    pub async fn create_signatures(&self, abi: &str) -> Result<(), anyhow::Error> {
        let sources = self.sources.clone();
        let _responses = proxy!(sources, abi, create_signatures);
        Ok(())
    }

    pub async fn get_function_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        let sources = &self.sources;
        let responses = proxy!(sources, hex, get_function_signatures);
        let signatures = Self::merge_signatures(responses);
        Ok(signatures)
    }

    pub async fn get_event_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error> {
        let sources = &self.sources;
        let responses = proxy!(sources, hex, get_event_signatures);
        let signatures = Self::merge_signatures(responses);
        Ok(signatures)
    }

    pub async fn get_function_abi(&self, tx_input: &[u8]) -> Result<Abi, anyhow::Error> {
        if tx_input.len() < 4 {
            anyhow::bail!("tx input len must be at least 4 bytes");
        }
        let hex_sig = hex::encode(&tx_input[..4]);
        let sigs = self.get_function_signatures(&hex_sig).await?;
        let found_signatures = sigs.len();
        sigs.into_iter()
            .filter_map(|sig| {
                let (name, args) = parse_signature(&sig)?;
                let values = decode_txinput(&args, &tx_input[4..])?;
                let inputs = parse_args("arg".into(), &args, &values);
                Some(Abi {
                    name: name.into(),
                    inputs,
                })
            })
            .next()
            .ok_or_else(|| {
                anyhow::Error::msg(
                    format!(
                        "could not find any signature that fits given tx input; found {} signatures, but could not fit arguments into any of them", 
                        found_signatures
                    )
                )
            })
    }

    pub async fn get_event_abi(&self, raw: RawLog) -> Result<Abi, anyhow::Error> {
        if raw.topics.is_empty() {
            anyhow::bail!("log should contain at least one topic");
        }
        let hex_sig = hex::encode(raw.topics[0].as_bytes());
        let sigs = self.get_event_signatures(&hex_sig).await?;
        let found_signatures = sigs.len();
        sigs.into_iter()
            .filter_map(|sig| {
                let (name, args) = parse_signature(&sig)?;
                let values = decode_log(name.to_string(), &args, raw.clone())?;
                let inputs = parse_args("arg".into(), &args, &values);
                Some(Abi {
                    name: name.into(),
                    inputs,
                })
            })
            .next()
            .ok_or_else(|| {
                anyhow::Error::msg(
                    format!(
                        "could not find any signature that fits given log; found {} signatures, but could not fit arguments into any of them", 
                        found_signatures
                    )
                )
            })
    }
}

fn parse_signature(sig: &str) -> Option<(&str, Vec<ParamType>)> {
    let start = sig.find('(')?;
    let name = &sig[..start];
    let sig = &sig[start..];
    ethabi::param_type::Reader::read(sig)
        .ok()
        .and_then(|param| match param {
            ParamType::Tuple(params) => Some(params),
            _ => None,
        })
        .map(|params| (name, params))
}

fn decode_txinput(args: &[ParamType], tx_args: &[u8]) -> Option<Vec<Token>> {
    let decoded = ethabi::decode(args, tx_args).ok()?;

    // decode will not fail if it decodes only part of the input data
    // so we will encode the result and check, that we decoded the whole data
    let encoded = ethabi::encode(&decoded);
    if tx_args != encoded {
        return None;
    }
    Some(decoded)
}

fn decode_log(name: String, args: &[ParamType], raw: RawLog) -> Option<Vec<Token>> {
    const MAX_PERMUTATIONS: usize = 10000;
    // this is indeed can be very long
    // there are better ways to iterate over valid indexed permutations
    // but right now we think this is okayish
    for (ind, perm) in (0..raw.topics.len() - 1)
        .into_iter()
        .map(|_| true)
        .chain((0..args.len() - raw.topics.len() + 1).map(|_| false))
        .permutations(args.len())
        .enumerate()
        .dedup_by(|x, y| x.1 == y.1)
    {
        if ind > MAX_PERMUTATIONS {
            break;
        }
        let inputs = args
            .iter()
            .zip(perm)
            .enumerate()
            .map(|(ind, (param, indexed))| EventParam {
                name: format!("{}", ind),
                kind: param.clone(),
                indexed,
            })
            .collect();
        let event = Event {
            name: name.clone(),
            inputs,
            anonymous: false,
        };
        let tokens = event
            .parse_log(raw.clone())
            .ok()
            .map(|log| log.params.into_iter().map(|param| param.value).collect());
        if let Some(tokens) = tokens {
            return Some(tokens);
        }
    }
    None
}

fn parse_arg(name: String, param: &ParamType, value: &Token) -> Argument {
    let components = match (param, value) {
        (ParamType::Tuple(param), Token::Tuple(value)) => {
            parse_args(format!("{}_", name), param, value)
        }
        _ => Default::default(),
    };
    Argument {
        name,
        r#type: param.to_string(),
        components,
        value: value.to_string(),
    }
}

fn parse_args(pref: String, args: &[ParamType], values: &[Token]) -> Vec<Argument> {
    let inputs = args
        .iter()
        .zip(values.iter())
        .enumerate()
        .map(|(index, (arg, value))| parse_arg(format!("{}{}", pref, index), arg, value))
        .collect();
    inputs
}

#[cfg(test)]
mod tests {
    use crate::sources::MockSignatureSource;

    use super::*;
    use ethabi::ethereum_types::{H160, H256, U256};
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn function() {
        let tests = vec![
            (
                "70a0823100000000000000000000000000000000219ab540356cbb839cbe05303d7705fa",
                Abi {
                    name: "balanceOf".into(),
                    inputs: vec![Argument {
                        name: "arg0".into(),
                        r#type: "address".into(),
                        components: vec![],
                        value: "00000000219ab540356cbb839cbe05303d7705fa".into(),
                    }],
                },
            ),
            (
                "70a082310000000000000000000000000000000000000000000000000000000000bc61591234567812345678000000000000000000000000000000000000000000000000",
                Abi {
                    name: "branch_passphrase_public".into(),
                    inputs: vec![Argument {
                        name: "arg0".into(),
                        r#type: "uint256".into(),
                        components: vec![],
                        value: "bc6159".into(), // hex number 123456789
                    }, Argument {
                        name: "arg1".into(),
                        r#type: "bytes8".into(),
                        components: vec![],
                        value: "1234567812345678".into(),
                    }],
                },
            ),
            (
                "70a082310000000000000000000000000000000000000000000000000000000000bc615900000000000000000000000000000000219ab540356cbb839cbe05303d7705fa",
                Abi {
                    name: "passphrase_calculate_transfer".into(),
                    inputs: vec![Argument {
                        name: "arg0".into(),
                        r#type: "uint64".into(),
                        components: vec![],
                        value: "bc6159".into(), // hex number 123456789
                    }, Argument {
                        name: "arg1".into(),
                        r#type: "address".into(),
                        components: vec![],
                        value: "00000000219ab540356cbb839cbe05303d7705fa".into(),
                    }],
                },
            ),
        ];

        for (input, abi) in tests {
            let mut source = MockSignatureSource::new();
            source
                .expect_get_function_signatures()
                .with(mockall::predicate::eq("70a08231"))
                .times(1)
                .returning(|_| {
                    Ok(vec![
                        "balanceOf(address)".into(),
                        "branch_passphrase_public(uint256,bytes8)".into(),
                        "passphrase_calculate_transfer(uint64,address)".into(),
                    ])
                });
            let source = Arc::new(source);

            let agg = Arc::new(SourceAggregator::new(vec![source.clone()]));

            let function = agg
                .get_function_abi(&hex::decode(input).unwrap())
                .await
                .unwrap();
            assert_eq!(abi, function);
        }
    }

    fn encode_tuple() -> String {
        use ethabi::Token::*;
        let res = ethabi::encode(&vec![
            Tuple(vec![
                Uint(U256::from_dec_str("123456789").unwrap()),
                Address(H160(
                    hex::decode("00000000219ab540356cbb839cbe05303d7705fa")
                        .unwrap()
                        .try_into()
                        .unwrap(),
                )),
                Bytes(vec![123]),
            ]),
            Tuple(vec![
                Uint(U256::from_dec_str("123").unwrap()),
                FixedArray(vec![
                    FixedBytes(vec![11, 12, 13, 14]),
                    FixedBytes(vec![101, 102, 103, 104]),
                ]),
            ]),
        ]);
        hex::encode(&res)
    }

    #[tokio::test]
    async fn function_tuple() {
        let encoded = encode_tuple();
        let input = "68705463".to_string() + &encoded;

        let mut source = MockSignatureSource::new();
        source
            .expect_get_function_signatures()
            .with(mockall::predicate::eq("68705463"))
            .times(1)
            .returning(|_| {
                Ok(vec![
                    "test((uint256,address,bytes),(uint8,bytes32[2]))".into()
                ])
            });
        let source = Arc::new(source);

        let agg = Arc::new(SourceAggregator::new(vec![source.clone()]));

        let function = agg
            .get_function_abi(&hex::decode(input).unwrap())
            .await
            .unwrap();

        let expected = Abi {
            name: "test".into(),
            inputs: vec![
                Argument {
                    name: "arg0".into(),
                    r#type: "(uint256,address,bytes)".into(),
                    components: vec![
                        Argument {
                            name: "arg0_0".into(),
                            r#type: "uint256".into(),
                            components: vec![],
                            value: "75bcd15".into(),
                        },
                        Argument {
                            name: "arg0_1".into(),
                            r#type: "address".into(),
                            components: vec![],
                            value: "00000000219ab540356cbb839cbe05303d7705fa".into(),
                        },
                        Argument {
                            name: "arg0_2".into(),
                            r#type: "bytes".into(),
                            components: vec![],
                            value: "7b".into(),
                        },
                    ],
                    value: "(75bcd15,00000000219ab540356cbb839cbe05303d7705fa,7b)".into(),
                },
                Argument {
                    name: "arg1".into(),
                    r#type: "(uint8,bytes32[2])".into(),
                    components: vec![
                        Argument {
                            name: "arg1_0".into(),
                            r#type: "uint8".into(),
                            components: vec![],
                            value: "7b".into(),
                        },
                        Argument {
                            name: "arg1_1".into(),
                            r#type: "bytes32[2]".into(),
                            components: vec![],
                            value: "[0b0c0d0e00000000000000000000000000000000000000000000000000000000,6566676800000000000000000000000000000000000000000000000000000000]".into(),
                        },
                    ],
                    value: "(7b,[0b0c0d0e00000000000000000000000000000000000000000000000000000000,6566676800000000000000000000000000000000000000000000000000000000])".into(),
                },
            ],
        };
        assert_eq!(expected, function);
    }

    #[tokio::test]
    async fn event() {
        let tests = vec![
            (
                RawLog {
                    data: hex::decode(
                        "00000000000000000000000000000000000000000000000000000000006acfc0",
                    )
                    .unwrap(),
                    topics: vec![
                        H256::from_slice(
                            &hex::decode(
                                "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                            )
                            .unwrap(),
                        ),
                        H256::from_slice(
                            &hex::decode(
                                "000000000000000000000000b8ace4d9bc469ddc8e788e636e817c299a1a8150",
                            )
                            .unwrap(),
                        ),
                        H256::from_slice(
                            &hex::decode(
                                "000000000000000000000000f76c5b19e86c256482f4aad1dae620a0c3ac0cd6",
                            )
                            .unwrap(),
                        ),
                    ],
                },
                "Transfer(address,address,uint256)",
                Abi {
                    name: "Transfer".into(),
                    inputs: vec![
                        Argument {
                            name: "arg0".into(),
                            r#type: "address".into(),
                            components: vec![],
                            value: "b8ace4d9bc469ddc8e788e636e817c299a1a8150".into(),
                        },
                        Argument {
                            name: "arg1".into(),
                            r#type: "address".into(),
                            components: vec![],
                            value: "f76c5b19e86c256482f4aad1dae620a0c3ac0cd6".into(),
                        },
                        Argument {
                            name: "arg2".into(),
                            r#type: "uint256".into(),
                            components: vec![],
                            value: "6acfc0".into(),
                        },
                    ],
                },
            ),
            (
                RawLog {
                    data: hex::decode(
                        "0000000000000000000000000000000000000000000000083ed9ef578babdb5c00000000000000000000000000000000000000000000bf05c05e3ce0f57a7e39",
                    )
                    .unwrap(),
                    topics: vec![
                        H256::from_slice(
                            &hex::decode(
                                "1c411e9a96e071241c2f21f7726b17ae89e3cab4c78be50e062b03a9fffbbad1",
                            )
                            .unwrap(),
                        ),
                    ],
                },
                "Sync(uint112,uint112)",
                Abi {
                    name: "Sync".into(),
                    inputs: vec![
                        Argument {
                            name: "arg0".into(),
                            r#type: "uint112".into(),
                            components: vec![],
                            value: "83ed9ef578babdb5c".into(),
                        },
                        Argument {
                            name: "arg1".into(),
                            r#type: "uint112".into(),
                            components: vec![],
                            value: "bf05c05e3ce0f57a7e39".into(),
                        },
                    ],
                },
            ),
            (
                RawLog {
                    data: hex::decode(
                        "000000000000000000000000000000000000000000000000030d98d59a960000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000469ef5a473f28f3a21",
                    )
                    .unwrap(),
                    topics: vec![
                        H256::from_slice(
                            &hex::decode(
                                "d78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822",
                            )
                            .unwrap(),
                        ),
                        H256::from_slice(
                            &hex::decode(
                                "00000000000000000000000068b3465833fb72a70ecdf485e0e4c7bd8665fc45",
                            )
                            .unwrap(),
                        ),
                        // this is actually last argument, but we can't determine that just from signature
                        // so we decode it as second argument
                        H256::from_slice(
                            &hex::decode(
                                "000000000000000000000000220575d6e7e8797ad18d0d660c7e1ecf4e1a1ed1",
                            )
                            .unwrap(),
                        ),
                    ],
                },
                "Swap(address,uint256,uint256,uint256,uint256,address)",
                Abi {
                    name: "Swap".into(),
                    inputs: vec![
                        Argument {
                            name: "arg0".into(),
                            r#type: "address".into(),
                            components: vec![],
                            value: "68b3465833fb72a70ecdf485e0e4c7bd8665fc45".into(),
                        },
                        Argument {
                            name: "arg1".into(),
                            r#type: "uint256".into(),
                            components: vec![],
                            value: "220575d6e7e8797ad18d0d660c7e1ecf4e1a1ed1".into(),
                        },
                        Argument {
                            name: "arg2".into(),
                            r#type: "uint256".into(),
                            components: vec![],
                            value: "30d98d59a960000".into(),
                        },
                        Argument {
                            name: "arg3".into(),
                            r#type: "uint256".into(),
                            components: vec![],
                            value: "0".into(),
                        },
                        Argument {
                            name: "arg4".into(),
                            r#type: "uint256".into(),
                            components: vec![],
                            value: "0".into(),
                        },
                        Argument {
                            name: "arg5".into(),
                            r#type: "address".into(),
                            components: vec![],
                            value: "0000000000000000000000469ef5a473f28f3a21".into(),
                        },
                    ],
                },
            ),
        ];
        for (input, sig, abi) in tests {
            let expected = hex::encode(input.topics[0].as_bytes());
            let mut source = MockSignatureSource::new();
            source
                .expect_get_event_signatures()
                .withf(move |hex| hex == expected)
                .times(1)
                .returning(|_| Ok(vec![sig.into()]));
            let source = Arc::new(source);

            let agg = Arc::new(SourceAggregator::new(vec![source.clone()]));

            let event = agg.get_event_abi(input).await.unwrap();
            dbg!(&event);
            assert_eq!(abi, event);
        }
    }
}
