use crate::SignatureSource;
use ethabi::ParamType;
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
        sigs.into_iter()
            .filter_map(|sig| {
                parse_signature(&sig).and_then(|(name, args)| {
                    parse_args(&args, &tx_input[4..]).map(|inputs| Abi {
                        name: name.into(),
                        inputs,
                    })
                })
            })
            .next()
            .ok_or_else(|| {
                anyhow::Error::msg("could not find any signature that fits given tx input")
            })
    }

    pub async fn get_event_abi(
        &self,
        _data: String,
        _topics: Vec<String>,
    ) -> Result<Abi, anyhow::Error> {
        anyhow::bail!("unimplemented")
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

fn parse_args(args: &[ParamType], tx_args: &[u8]) -> Option<Vec<Argument>> {
    let decoded = ethabi::decode(args, tx_args).ok()?;

    // decode will not fail if it decodes only part of the input data
    // so we will encode the result and check, that we decoded the whole data
    let encoded = ethabi::encode(&decoded);
    if tx_args != encoded {
        return None;
    }

    let inputs = args
        .iter()
        .zip(decoded.into_iter())
        .enumerate()
        .map(|(index, (param, value))| Argument {
            name: format!("arg{}", index),
            r#type: param.to_string(),
            components: Default::default(),
            value: value.to_string(),
        })
        .collect();
    Some(inputs)
}

#[cfg(test)]
mod tests {
    use crate::sources::MockSignatureSource;

    use super::*;
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
}
