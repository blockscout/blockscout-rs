use anyhow::Context;
use serde::Deserialize;

pub use cbor_auxdata::{parse as parse_cbor_auxdata, CborAuxdata};
mod cbor_auxdata {
    use super::*;
    use blockscout_display_bytes::Bytes as DisplayBytes;
    use std::collections::BTreeMap;

    pub type CborAuxdata = BTreeMap<String, CborAuxdataValue>;

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct CborAuxdataValue {
        pub offset: usize,
        pub value: DisplayBytes,
    }

    pub fn parse(code_artifacts: serde_json::Value) -> Result<CborAuxdata, anyhow::Error> {
        #[derive(Clone, Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "camelCase")]
        struct CodeArtifacts {
            #[serde(default)]
            pub cbor_auxdata: CborAuxdata,
        }

        let code_artifacts = serde_json::from_value::<CodeArtifacts>(code_artifacts)
            .context("code artifacts deserialization failed")?;
        Ok(code_artifacts.cbor_auxdata)
    }
}

pub use immutable_references::{parse as parse_immutable_references, ImmutableReferences};
mod immutable_references {
    use super::*;
    use std::collections::BTreeMap;

    pub type ImmutableReferences = BTreeMap<String, Vec<ImmutableReferenceValue>>;

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct ImmutableReferenceValue {
        pub length: usize,
        pub start: usize,
    }

    pub fn parse(code_artifacts: serde_json::Value) -> Result<ImmutableReferences, anyhow::Error> {
        #[derive(Clone, Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "camelCase")]
        struct CodeArtifacts {
            #[serde(default)]
            pub immutable_references: ImmutableReferences,
        }

        let code_artifacts = serde_json::from_value::<CodeArtifacts>(code_artifacts)
            .context("code artifacts deserialization failed")?;
        Ok(code_artifacts.immutable_references)
    }
}

pub use link_references::{parse as parse_link_references, LinkReferences};
mod link_references {
    use super::*;
    use std::collections::BTreeMap;

    pub type LinkReferences = BTreeMap<String, BTreeMap<String, Vec<LinkReferenceValue>>>;

    #[derive(Clone, Debug, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct LinkReferenceValue {
        pub length: usize,
        pub start: usize,
    }

    pub fn parse(code_artifacts: serde_json::Value) -> Result<LinkReferences, anyhow::Error> {
        #[derive(Clone, Debug, Deserialize, PartialEq)]
        #[serde(rename_all = "camelCase")]
        struct CodeArtifacts {
            #[serde(default)]
            pub link_references: LinkReferences,
        }

        let code_artifacts = serde_json::from_value::<CodeArtifacts>(code_artifacts)
            .context("code artifacts deserialization failed")?;
        Ok(code_artifacts.link_references)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cbor_auxdata::CborAuxdataValue, immutable_references::ImmutableReferenceValue,
        link_references::LinkReferenceValue, *,
    };
    use blockscout_display_bytes::Bytes as DisplayBytes;
    use pretty_assertions::assert_eq;
    use std::{collections::BTreeMap, str::FromStr};

    #[test]
    fn test_parse_cbor_auxdata() {
        let code_artifacts = serde_json::json!({
            "cborAuxdata": {
                "1": {
                    "offset": 1639,
                    "value": "0xa264697066735822122005d1b64ca59de3c6d96eee72b6fef65fc503bfbf8d9719fb047fafce2ebdc29764736f6c63430008120033"
                },
                "2": {
                    "offset": 1731,
                    "value": "0xa2646970667358221220aebf48746b808da25305449bba6945baacf1c2185dfcc58a94b1506b8b5a6dfa64736f6c63430008120033"
                }
            }
        });

        let expected = BTreeMap::from([
            ("1".to_string(),
            CborAuxdataValue {
                offset: 1639,
                value: DisplayBytes::from_str("0xa264697066735822122005d1b64ca59de3c6d96eee72b6fef65fc503bfbf8d9719fb047fafce2ebdc29764736f6c63430008120033").unwrap(),
            }),
            ("2".to_string(),
            CborAuxdataValue {
                offset: 1731,
                value: DisplayBytes::from_str("0xa2646970667358221220aebf48746b808da25305449bba6945baacf1c2185dfcc58a94b1506b8b5a6dfa64736f6c63430008120033").unwrap(),
            })
        ]);
        let actual = parse_cbor_auxdata(code_artifacts).expect("parsing failed");

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_parse_immutable_references() {
        let code_artifacts = serde_json::json!({
            "immutableReferences": {
                "7":[{"length":32,"start":176}, {"length":32,"start":255}],
                "10":[{"length":32,"start":101}]
            },
        });

        let expected = BTreeMap::from([
            (
                "7".to_string(),
                vec![
                    ImmutableReferenceValue {
                        length: 32,
                        start: 176,
                    },
                    ImmutableReferenceValue {
                        length: 32,
                        start: 255,
                    },
                ],
            ),
            (
                "10".to_string(),
                vec![ImmutableReferenceValue {
                    length: 32,
                    start: 101,
                }],
            ),
        ]);
        let actual = parse_immutable_references(code_artifacts).expect("parsing failed");

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_parse_link_references() {
        let code_artifacts = serde_json::json!({
            "linkReferences": {"contracts/1_Storage.sol":{"Journal":[{"length":20,"start":185}]}}
        });

        let expected = BTreeMap::from([(
            "contracts/1_Storage.sol".to_string(),
            BTreeMap::from([(
                "Journal".to_string(),
                vec![LinkReferenceValue {
                    length: 20,
                    start: 185,
                }],
            )]),
        )]);
        let actual = parse_link_references(code_artifacts).expect("parsing failed");

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_parse_with_absent_artifacts() {
        let code_artifacts = serde_json::json!({});

        /********** Cbor auxdata **********/

        let expected = CborAuxdata::default();
        let actual =
            parse_cbor_auxdata(code_artifacts.clone()).expect("parsing of cbor auxdata failed");
        assert_eq!(expected, actual, "invalid cbor auxdata");

        /********** Immutable references  **********/

        let expected = ImmutableReferences::default();
        let actual = parse_immutable_references(code_artifacts.clone())
            .expect("parsing of immutable references failed");
        assert_eq!(expected, actual, "invalid immutable references");

        /********** Link references  **********/

        let expected = LinkReferences::default();
        let actual = parse_link_references(code_artifacts.clone())
            .expect("parsing of link references failed");
        assert_eq!(expected, actual, "invalid link references");
    }
}
