// TODO: try move to common crate since code is copipasted from smart-contract-verifier

use crate::verification::MatchType;
use blockscout_display_bytes::Bytes as DisplayBytes;
use bytes::Bytes;
use entity::{parts, sea_orm_active_enums::PartType};
use ethabi::{Constructor, Token};
use mismatch::Mismatch;
use solidity_metadata::MetadataHash;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BytecodePart {
    Main {
        raw: Bytes,
    },
    Metadata {
        raw: Bytes,
        metadata: MetadataHash,
        metadata_length_raw: Bytes,
    },
}

impl TryFrom<&parts::Model> for BytecodePart {
    type Error = anyhow::Error;

    fn try_from(part: &parts::Model) -> Result<Self, Self::Error> {
        let part = match part.part_type {
            PartType::Main => Self::Main {
                raw: Bytes::copy_from_slice(&part.data),
            },
            PartType::Metadata => {
                let (metadata, length) = MetadataHash::from_cbor(&part.data)?;
                let metadata_length_raw = &part.data[length..];
                Self::Metadata {
                    raw: Bytes::copy_from_slice(&part.data),
                    metadata,
                    metadata_length_raw: Bytes::copy_from_slice(metadata_length_raw),
                }
            }
        };
        Ok(part)
    }
}

impl BytecodePart {
    pub fn raw(&self) -> &Bytes {
        match self {
            BytecodePart::Main { raw } => raw,
            BytecodePart::Metadata { raw, .. } => raw,
        }
    }

    pub fn size(&self) -> usize {
        self.raw().len()
    }
}

pub struct LocalBytecode {
    pub parts: Vec<BytecodePart>,
}

impl LocalBytecode {
    pub fn new(parts: &[parts::Model]) -> Result<Self, anyhow::Error> {
        Ok(Self {
            parts: parts
                .iter()
                .map(BytecodePart::try_from)
                .collect::<Result<_, _>>()?,
        })
    }

    pub fn raw_bytecode(&self) -> Bytes {
        Bytes::from_iter(self.parts.iter().flat_map(|p| p.raw().to_vec()))
    }
}

#[derive(Error, Clone, Debug, PartialEq, Eq)]
pub enum CompareError {
    #[error("bytecode length is less than expected: {part}; bytecodes: {raw}")]
    BytecodeLengthMismatch {
        part: Mismatch<usize>,
        raw: Mismatch<DisplayBytes>,
    },
    #[error("bytecode does not match compilation output: {part}; bytecodes: {raw}")]
    BytecodeMismatch {
        part: Mismatch<DisplayBytes>,
        raw: Mismatch<DisplayBytes>,
    },
    #[error("cannot parse metadata")]
    MetadataParse(String),
    #[error("compiler versions included into metadata hash does not match: {0}")]
    CompilerVersionMismatch(Mismatch<semver::Version>),
    #[error("invalid constructor arguments: {0}")]
    InvalidConstructorArguments(DisplayBytes),
}

pub fn compare(remote_bytecode: &Bytes, local: &LocalBytecode) -> Result<MatchType, CompareError> {
    let local_bytecode = &local.raw_bytecode();

    if remote_bytecode.starts_with(local_bytecode) {
        // If local compilation bytecode is prefix of remote one,
        // metadata parts are the same and we do not need to compare bytecode parts.
        return Ok(MatchType::Full);
    }

    if remote_bytecode.len() < local_bytecode.len() {
        return Err(CompareError::BytecodeLengthMismatch {
            part: Mismatch::new(local_bytecode.len(), remote_bytecode.len()),
            raw: Mismatch::new(
                local_bytecode.clone().into(),
                remote_bytecode.clone().into(),
            ),
        });
    }
    compare_bytecode_parts(remote_bytecode, local_bytecode, &local.parts)?;

    Ok(MatchType::Partial)
}

pub fn extract_constructor_args(
    remote_raw: &Bytes,
    local_raw: &Bytes,
    abi_constructor: Option<&Constructor>,
    is_creation_input: bool,
) -> Result<Option<Bytes>, CompareError> {
    let encoded_constructor_args = remote_raw.slice(local_raw.len()..);
    let encoded_constructor_args = if encoded_constructor_args.is_empty() {
        None
    } else {
        Some(encoded_constructor_args)
    };

    let expects_constructor_args = is_creation_input // check that the source actually should have constructor args
            && abi_constructor.map(|input| input.inputs.len()).unwrap_or(0) > 0; // check that the contract itself should have constructor args

    match encoded_constructor_args {
        None if expects_constructor_args => Err(CompareError::InvalidConstructorArguments(
            DisplayBytes::from([]),
        )),
        Some(encoded) if !expects_constructor_args => {
            Err(CompareError::InvalidConstructorArguments(encoded.into()))
        }
        None => Ok(None),
        Some(encoded_constructor_args) => {
            let _constructor_args = parse_constructor_args(
                encoded_constructor_args.clone(),
                abi_constructor.expect("Is not None as `expects_constructor_args`"),
            )?;
            Ok(Some(encoded_constructor_args))
        }
    }
}

fn compare_bytecode_parts(
    remote_raw: &Bytes,
    local_raw: &Bytes,
    local_parts: &Vec<BytecodePart>,
) -> Result<(), CompareError> {
    // A caller should ensure that this precondition holds.
    // Currently only `compare_creation_tx_inputs` calls current function,
    // and it guarantees that `remote_creation_tx_input.len() < local_creation_tx_input.len()`
    assert!(
        // if that fails, we would be out of range further anyway
        remote_raw.len() >= local_raw.len(),
        "Local bytecode is greater than remote"
    );

    let mut i = 0usize; // keep track of current processing position of `remote_raw`

    for part in local_parts {
        match part {
            BytecodePart::Main { raw } => {
                if raw != &remote_raw[i..i + raw.len()] {
                    return Err(CompareError::BytecodeMismatch {
                        part: Mismatch::new(
                            raw.clone().into(),
                            remote_raw.slice(i..i + raw.len()).into(),
                        ),
                        raw: Mismatch::new(local_raw.clone().into(), remote_raw.clone().into()),
                    });
                }
            }
            BytecodePart::Metadata {
                metadata,
                metadata_length_raw,
                ..
            } => {
                let (remote_metadata, remote_metadata_length) =
                    MetadataHash::from_cbor(&remote_raw[i..])
                        .map_err(|err| CompareError::MetadataParse(err.to_string()))?;
                let start_index = i + remote_metadata_length;
                if remote_raw.len() <= start_index {
                    return Err(CompareError::MetadataParse(
                        "metadata doesn't have encoded length".into(),
                    ));
                }
                if &remote_raw[start_index..start_index + 2] != metadata_length_raw {
                    return Err(CompareError::MetadataParse(
                        "metadata length mismatch".into(),
                    ));
                }

                // We may say the compiler versions does not correspond to each other only in case if both compiler versions are present.
                // Otherwise, we cannot say for sure if compiler version is invalid.
                if let (Some(metadata_solc), Some(remote_metadata_solc)) =
                    (&metadata.solc, &remote_metadata.solc)
                {
                    if metadata_solc != remote_metadata_solc {
                        let expected_solc = metadata_solc.clone();
                        let remote_solc = remote_metadata_solc.clone();
                        return Err(CompareError::CompilerVersionMismatch(Mismatch::new(
                            expected_solc,
                            remote_solc,
                        )));
                    }
                }
            }
        }

        i += part.size();
    }

    Ok(())
}

fn parse_constructor_args(
    encoded_args: Bytes,
    abi_constructor: &Constructor,
) -> Result<Vec<Token>, CompareError> {
    let param_types = |inputs: &Vec<ethabi::Param>| -> Vec<ethabi::ParamType> {
        inputs.iter().map(|p| p.kind.clone()).collect()
    };
    let param_types = param_types(&abi_constructor.inputs);
    let tokens = ethabi::decode(&param_types, encoded_args.as_ref())
        .map_err(|_err| CompareError::InvalidConstructorArguments(encoded_args.into()))?;

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use semver::Version;
    use std::str::FromStr;

    const DEFAULT_MAIN: &str = "6080604052348015600f57600080fd5b506004361060285760003560e01c8063f43fa80514602d575b600080fd5b60336047565b604051603e91906062565b60405180910390f35b600065100000000001905090565b605c81607b565b82525050565b6000602082019050607560008301846055565b92915050565b600081905091905056fe";
    const DEFAULT_META: &str = "a2646970667358221220ad5a5e9ea0429c6665dc23af78b0acca8d56235be9dc3573672141811ea4a0da64736f6c63430008070033";

    #[test]
    fn db_convert() {
        let main = parts::Model {
            id: 0,
            part_type: PartType::Main,
            data: hex::decode(DEFAULT_MAIN).unwrap(),
            data_text: DEFAULT_MAIN.to_string(),
            created_at: Default::default(),
            updated_at: Default::default(),
        };

        let part = BytecodePart::try_from(&main).expect("cannot convert main bytecode");
        match part {
            BytecodePart::Main { raw } => {
                assert_eq!(raw.to_vec(), main.data,);
            }
            BytecodePart::Metadata { .. } => panic!("invalid type for bytecode part"),
        };

        let meta = parts::Model {
            id: 0,
            part_type: PartType::Metadata,
            data: hex::decode(DEFAULT_META).unwrap(),
            data_text: DEFAULT_META.to_string(),
            created_at: Default::default(),
            updated_at: Default::default(),
        };

        let part = BytecodePart::try_from(&meta).expect("cannot convert meta bytecode");
        match part {
            BytecodePart::Main { .. } => {
                panic!("invalid type for bytecode part");
            }
            BytecodePart::Metadata {
                raw,
                metadata,
                metadata_length_raw,
            } => {
                assert_eq!(raw.to_vec(), meta.data,);
                assert_eq!(
                    metadata.solc,
                    Some(Version::from_str("0.8.7").expect("valid semver"))
                );
                let length = 0x33;
                assert_eq!(metadata_length_raw.to_vec(), vec![0x0, length]);
                assert_eq!(raw.len() - 2, length as usize);
            }
        };
    }

    fn get_parts(bytecodes: &[&str]) -> Vec<parts::Model> {
        bytecodes
            .iter()
            .enumerate()
            .map(|(i, bytecode)| {
                let part_type = if i % 2 == 0 {
                    PartType::Main
                } else {
                    PartType::Metadata
                };
                parts::Model {
                    id: i as i64,
                    part_type,
                    data: hex::decode(bytecode).unwrap(),
                    data_text: bytecode.to_string(),
                    created_at: Default::default(),
                    updated_at: Default::default(),
                }
            })
            .collect()
    }

    fn test_compare(remote: &str, bytecodes: Vec<&str>, eq: bool) {
        let parts = get_parts(&bytecodes);
        let remote = DisplayBytes::from_str(remote).unwrap().0;
        let local = LocalBytecode::new(&parts).unwrap();

        let result = compare(&remote, &local);
        if eq {
            result.expect("expected eq bytecodes, got error");
        } else {
            result.expect_err("expected error during bytecodes compare, but got eq");
        }
    }

    #[test]
    fn compare_same() {
        let bytecodes = vec![DEFAULT_MAIN, DEFAULT_META];
        test_compare(&bytecodes.join(""), bytecodes, true);
    }

    #[test]
    fn compare_diff_meta() {
        let bytecodes = vec![DEFAULT_MAIN, DEFAULT_META];
        let remote = format!("{}{}", 
            DEFAULT_MAIN,
            "a2646970667358221220940dbafd63b6b52884aa9499b7b61e99e33685af80e603ffe485e9efe2ac2f7764736f6c63430008070033"
        );
        test_compare(&remote, bytecodes, true);
    }

    #[test]
    fn compare_diff_main() {
        for random_string in [
            "",
            "6080",
            &format!("{DEFAULT_MAIN}53"),
            &format!("6080{DEFAULT_MAIN}"),
        ] {
            let bytecodes = vec![DEFAULT_MAIN, DEFAULT_META];
            let remote = format!("{random_string}{DEFAULT_META}");
            test_compare(&remote, bytecodes, false);
        }
    }

    #[test]
    fn compare_same_double_meta() {
        let bytecodes = vec![DEFAULT_MAIN, DEFAULT_META, DEFAULT_MAIN, DEFAULT_META];
        test_compare(&bytecodes.join(""), bytecodes, true);
    }

    #[test]
    fn compare_diff_meta_double_meta() {
        let bytecodes = vec![DEFAULT_MAIN, DEFAULT_META, DEFAULT_MAIN, DEFAULT_META];
        let remote = format!("{}{}{}{}", 
            DEFAULT_MAIN,
            "a2646970667358221220940dbafd63b6b52884aa9499b7b61e99e33685af80e603ffe485e9efe2ac2f7764736f6c63430008070033",
            DEFAULT_MAIN,
            "a2646970667358221220c424331e61ba143d01f757e1a3b6ddcfe99698f6c1862e2133c4d7d277854b9564736f6c63430008070033"
        );
        test_compare(&remote, bytecodes, true);
    }

    #[test]
    fn compare_diff_main_double_meta() {
        for (random_string1, random_string2) in [
            ("", ""),
            ("6080", "6080"),
            (&format!("{DEFAULT_MAIN}53"), &format!("{DEFAULT_MAIN}53")),
            (
                &format!("6080{DEFAULT_MAIN}"),
                &format!("6080{DEFAULT_MAIN}"),
            ),
        ] {
            let bytecodes = vec![DEFAULT_MAIN, DEFAULT_META, DEFAULT_MAIN, DEFAULT_META];
            let remote = format!("{}{}{}{}", 
                random_string1,
                "a2646970667358221220c424331e61ba143d01f757e1a3b6ddcfe99698f6c1862e2133c4d7d277854b9564736f6c63430008070033",
                random_string2,
                "608060405234801561001057600080fd5b5060bb8061001f6000396000f3fe6080604052348015600f57600080fd5b506004361060285760003560e01c8063f43fa80514602d575b600080fd5b60336047565b604051603e91906062565b60405180910390f35b600065100000000001905090565b605c81607b565b82525050565b6000602082019050607560008301846055565b92915050565b600081905091905056fea2646970667358221220c424331e61ba143d01f757e1a3b6ddcfe99698f6c1862e2133c4d7d277854b9564736f6c63430008070033",
            );
            test_compare(&remote, bytecodes, false);
        }
    }
}
