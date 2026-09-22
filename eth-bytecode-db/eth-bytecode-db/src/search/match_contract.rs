// SPDX-License-Identifier: LicenseRef-Blockscout

use super::{
    bytecodes_comparison::extract_constructor_args,
    types::{BytecodeRemote, BytecodeType},
};
use crate::{verification, verification::SourceType};
use anyhow::Context;
use entity::{files, sea_orm_active_enums, source_files, sources};
use ethabi::Constructor;
use sea_orm::{
    prelude::{DateTime, DbErr},
    ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, JoinType, QueryFilter, QuerySelect,
    RelationTrait, Select,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use verification_common::solidity_libraries;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchContract {
    pub updated_at: DateTime,
    pub file_name: String,
    pub contract_name: String,
    pub compiler_version: String,
    pub compiler_settings: serde_json::Value,
    pub source_type: SourceType,
    pub source_files: BTreeMap<String, String>,
    pub abi: Option<String>,
    pub constructor_arguments: Option<String>,
    pub match_type: verification::MatchType,
    pub compilation_artifacts: Option<String>,
    pub creation_input_artifacts: Option<String>,
    pub deployed_bytecode_artifacts: Option<String>,
    pub is_blueprint: bool,
    pub libraries: BTreeMap<String, String>,
}

/// The `sources` columns a [`MatchContract`] is built from.
///
/// `sources` also stores both raw bytecodes, each a multi-kilobyte `bytea`. Neither is
/// part of the response, and the only thing the build needs out of them - the offset the
/// local bytecode ends at - is already known from the candidate's parts, so they are
/// deliberately left out of the projection.
#[derive(Clone, Debug, FromQueryResult)]
pub(super) struct SourceDetails {
    pub id: i64,
    pub updated_at: DateTime,
    pub source_type: sea_orm_active_enums::SourceType,
    pub compiler_version: String,
    pub compiler_settings: serde_json::Value,
    pub file_name: String,
    pub contract_name: String,
    pub abi: Option<serde_json::Value>,
    pub compilation_artifacts: Option<serde_json::Value>,
    pub creation_input_artifacts: Option<serde_json::Value>,
    pub deployed_bytecode_artifacts: Option<serde_json::Value>,
}

#[derive(Debug, FromQueryResult)]
struct SourceFile {
    source_id: i64,
    name: String,
    content: String,
}

fn source_details_query(source_ids: &[i64]) -> Select<sources::Entity> {
    sources::Entity::find()
        .select_only()
        .columns([
            sources::Column::Id,
            sources::Column::UpdatedAt,
            sources::Column::SourceType,
            sources::Column::CompilerVersion,
            sources::Column::CompilerSettings,
            sources::Column::FileName,
            sources::Column::ContractName,
            sources::Column::Abi,
            sources::Column::CompilationArtifacts,
            sources::Column::CreationInputArtifacts,
            sources::Column::DeployedBytecodeArtifacts,
        ])
        .filter(sources::Column::Id.is_in(source_ids.iter().copied()))
}

fn source_files_query(source_ids: &[i64]) -> Select<source_files::Entity> {
    source_files::Entity::find()
        .select_only()
        .column(source_files::Column::SourceId)
        .column(files::Column::Name)
        .column(files::Column::Content)
        .join(JoinType::InnerJoin, source_files::Relation::Files.def())
        .filter(source_files::Column::SourceId.is_in(source_ids.iter().copied()))
}

/// Retrieves the sources behind `source_ids` in a single query, keyed by source id.
pub(super) async fn find_source_details<C>(
    db: &C,
    source_ids: &[i64],
) -> Result<HashMap<i64, SourceDetails>, DbErr>
where
    C: ConnectionTrait,
{
    let sources = source_details_query(source_ids)
        .into_model::<SourceDetails>()
        .all(db)
        .await?;

    Ok(sources
        .into_iter()
        .map(|source| (source.id, source))
        .collect())
}

/// Retrieves the source files of `source_ids` in a single query, grouped by source id.
///
/// Keeping this apart from [`find_source_details`] is what stops the join from repeating
/// every source row - abi, compiler settings and all three artifact blobs included - once
/// per file that source has.
pub(super) async fn find_source_files<C>(
    db: &C,
    source_ids: &[i64],
) -> Result<HashMap<i64, BTreeMap<String, String>>, DbErr>
where
    C: ConnectionTrait,
{
    let source_files = source_files_query(source_ids)
        .into_model::<SourceFile>()
        .all(db)
        .await?;

    let mut grouped: HashMap<i64, BTreeMap<String, String>> = HashMap::new();
    for source_file in source_files {
        grouped
            .entry(source_file.source_id)
            .or_default()
            .insert(source_file.name, source_file.content);
    }

    Ok(grouped)
}

impl MatchContract {
    pub(super) fn build(
        source: SourceDetails,
        source_files: BTreeMap<String, String>,
        local_bytecode_len: usize,
        remote: &BytecodeRemote,
        match_type: verification::MatchType,
    ) -> Result<Self, anyhow::Error> {
        let constructor = get_constructor(source.abi.clone()).context("source has invalid abi")?;
        let has_constructor_args = remote.bytecode_type == BytecodeType::CreationCode;
        let constructor_args = extract_constructor_args(
            &remote.data,
            local_bytecode_len,
            constructor.as_ref(),
            has_constructor_args,
        )
        .map_err(|e| {
            tracing::error!("failed to extract constructor: {}", e);
            e
        })
        .context("invalid constructor arguments")?;
        let libraries =
            solidity_libraries::try_parse_compiler_linked_libraries(&source.compiler_settings)
                .inspect_err(|e| {
                    tracing::error!("failed to parse compiler linked libraries: {e:#?}");
                })?;
        let match_contract = MatchContract {
            updated_at: source.updated_at,
            file_name: source.file_name,
            contract_name: source.contract_name,
            compiler_version: source.compiler_version,
            compiler_settings: source.compiler_settings,
            source_type: source.source_type.into(),
            source_files,
            abi: source.abi.map(|abi| abi.to_string()),
            constructor_arguments: constructor_args.map(hex::encode),
            match_type,
            compilation_artifacts: source.compilation_artifacts.map(|value| value.to_string()),
            creation_input_artifacts: source
                .creation_input_artifacts
                .map(|value| value.to_string()),
            deployed_bytecode_artifacts: source
                .deployed_bytecode_artifacts
                .map(|value| value.to_string()),
            is_blueprint: false,
            libraries,
        };

        Ok(match_contract)
    }
}

fn get_constructor(
    abi: Option<serde_json::Value>,
) -> Result<Option<Constructor>, serde_json::Error> {
    match abi {
        Some(abi) => {
            let abi: ethers_core::abi::Abi = serde_json::from_value(abi)?;
            Ok(abi.constructor)
        }
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::MatchType;
    use blockscout_display_bytes::Bytes as DisplayBytes;
    use pretty_assertions::assert_eq;
    use sea_orm::{DbBackend, QueryTrait};
    use std::str::FromStr;

    /// Contract code:
    /// ```solidity
    /// pragma solidity ^0.8.7;
    /// contract Number {
    ///     uint public number;
    ///     string public str;
    ///
    ///     constructor(uint _number, string memory _str) {
    ///         number = _number;
    ///         str = _str;
    ///     }
    /// }
    /// ```
    const NUMBER_MAIN_PART: &str = "60806040523480156200001157600080fd5b506040516200084b3803806200084b833981810160405281019062000037919062000226565b8160008190555080600190816200004f9190620004cd565b505050620005b4565b6000604051905090565b600080fd5b600080fd5b6000819050919050565b62000081816200006c565b81146200008d57600080fd5b50565b600081519050620000a18162000076565b92915050565b600080fd5b600080fd5b6000601f19601f8301169050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052604160045260246000fd5b620000fc82620000b1565b810181811067ffffffffffffffff821117156200011e576200011d620000c2565b5b80604052505050565b60006200013362000058565b9050620001418282620000f1565b919050565b600067ffffffffffffffff821115620001645762000163620000c2565b5b6200016f82620000b1565b9050602081019050919050565b60005b838110156200019c5780820151818401526020810190506200017f565b60008484015250505050565b6000620001bf620001b98462000146565b62000127565b905082815260208101848484011115620001de57620001dd620000ac565b5b620001eb8482856200017c565b509392505050565b600082601f8301126200020b576200020a620000a7565b5b81516200021d848260208601620001a8565b91505092915050565b6000806040838503121562000240576200023f62000062565b5b6000620002508582860162000090565b925050602083015167ffffffffffffffff81111562000274576200027362000067565b5b6200028285828601620001f3565b9150509250929050565b600081519050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b60006002820490506001821680620002df57607f821691505b602082108103620002f557620002f462000297565b5b50919050565b60008190508160005260206000209050919050565b60006020601f8301049050919050565b600082821b905092915050565b6000600883026200035f7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff8262000320565b6200036b868362000320565b95508019841693508086168417925050509392505050565b6000819050919050565b6000620003ae620003a8620003a2846200006c565b62000383565b6200006c565b9050919050565b6000819050919050565b620003ca836200038d565b620003e2620003d982620003b5565b8484546200032d565b825550505050565b600090565b620003f9620003ea565b62000406818484620003bf565b505050565b5b818110156200042e5762000422600082620003ef565b6001810190506200040c565b5050565b601f8211156200047d576200044781620002fb565b620004528462000310565b8101602085101562000462578190505b6200047a620004718562000310565b8301826200040b565b50505b505050565b600082821c905092915050565b6000620004a26000198460080262000482565b1980831691505092915050565b6000620004bd83836200048f565b9150826002028217905092915050565b620004d8826200028c565b67ffffffffffffffff811115620004f457620004f3620000c2565b5b620005008254620002c6565b6200050d82828562000432565b600060209050601f83116001811462000545576000841562000530578287015190505b6200053c8582620004af565b865550620005ac565b601f1984166200055586620002fb565b60005b828110156200057f5784890151825560018201915060208501945060208101905062000558565b868310156200059f57848901516200059b601f8916826200048f565b8355505b6001600288020188555050505b505050505050565b61028780620005c46000396000f3fe608060405234801561001057600080fd5b50600436106100365760003560e01c80638381f58a1461003b578063c15bae8414610059575b600080fd5b610043610077565b6040516100509190610124565b60405180910390f35b61006161007d565b60405161006e91906101cf565b60405180910390f35b60005481565b6001805461008a90610220565b80601f01602080910402602001604051908101604052809291908181526020018280546100b690610220565b80156101035780601f106100d857610100808354040283529160200191610103565b820191906000526020600020905b8154815290600101906020018083116100e657829003601f168201915b505050505081565b6000819050919050565b61011e8161010b565b82525050565b60006020820190506101396000830184610115565b92915050565b600081519050919050565b600082825260208201905092915050565b60005b8381101561017957808201518184015260208101905061015e565b60008484015250505050565b6000601f19601f8301169050919050565b60006101a18261013f565b6101ab818561014a565b93506101bb81856020860161015b565b6101c481610185565b840191505092915050565b600060208201905081810360008301526101e98184610196565b905092915050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b6000600282049050600182168061023857607f821691505b60208210810361024b5761024a6101f1565b5b5091905056fe";
    const NUMBER_META_PART: &str = "a26469706673582212202ec25b2395cacdfaf72db8374301e337eda2a878ca3089a34d47f0cf8d2968fc64736f6c63430008110033";
    const NUMBER_ARGS_PART: &str = "00000000000000000000000000000000000000000000000000000101010101010000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000561626f6261000000000000000000000000000000000000000000000000000000";

    fn source() -> SourceDetails {
        SourceDetails {
            id: 1,
            source_type: sea_orm_active_enums::SourceType::Solidity,
            compiler_version: "v0.8.7".into(),
            compiler_settings: serde_json::json!({"settings": true}),
            file_name: "Number.sol".into(),
            contract_name: "Number".into(),
            abi: Some(
                serde_json::json!([ { "inputs": [ { "internalType": "uint256", "name": "_number", "type": "uint256" } ], "stateMutability": "nonpayable", "type": "constructor" }, { "inputs": [], "name": "number", "outputs": [ { "internalType": "uint256", "name": "", "type": "uint256" } ], "stateMutability": "view", "type": "function" } ]),
            ),
            updated_at: Default::default(),
            compilation_artifacts: Default::default(),
            creation_input_artifacts: Default::default(),
            deployed_bytecode_artifacts: Default::default(),
        }
    }

    /// The length of the local bytecode the candidate's parts add up to, which is where
    /// the remote bytecode stops being code and starts being constructor arguments.
    fn local_bytecode_len() -> usize {
        hex::decode(format!("{NUMBER_MAIN_PART}{NUMBER_META_PART}"))
            .unwrap()
            .len()
    }

    #[test]
    fn test_build_match_contract() {
        let source = source();
        let source_files =
            BTreeMap::from([("Number.sol".to_string(), "contract Number {}".to_string())]);

        let remote = BytecodeRemote {
            bytecode_type: BytecodeType::CreationCode,
            data: DisplayBytes::from_str(
                &[NUMBER_MAIN_PART, NUMBER_META_PART, NUMBER_ARGS_PART].join(""),
            )
            .unwrap()
            .0,
        };
        let result = MatchContract::build(
            source.clone(),
            source_files,
            local_bytecode_len(),
            &remote,
            verification::MatchType::Full,
        )
        .expect("unexpected error");

        assert_eq!(result.file_name, source.file_name);
        assert_eq!(result.contract_name, source.contract_name);
        assert_eq!(result.compiler_version, source.compiler_version);
        assert_eq!(result.compiler_settings, source.compiler_settings,);
        assert_eq!(result.source_type, source.source_type.into());
        assert_eq!(
            result.source_files,
            BTreeMap::from_iter([("Number.sol".to_string(), "contract Number {}".to_string())])
        );
        assert_eq!(result.abi, source.abi.map(|abi| abi.to_string()));
        assert_eq!(
            result.constructor_arguments.expect("args shoud be Some"),
            NUMBER_ARGS_PART,
        );
        assert_eq!(result.match_type, MatchType::Full);
    }

    #[test]
    fn test_build_match_contract_failed() {
        let invalid_args = "6080609001fe";
        let source = source();

        let remote = BytecodeRemote {
            bytecode_type: BytecodeType::CreationCode,
            data: DisplayBytes::from_str(
                &[NUMBER_MAIN_PART, NUMBER_META_PART, invalid_args].join(""),
            )
            .unwrap()
            .0,
        };
        let _ = MatchContract::build(
            source,
            BTreeMap::new(),
            local_bytecode_len(),
            &remote,
            verification::MatchType::Full,
        )
        .expect_err("expected error during decoding constructor arguments");
    }

    /// `raw_creation_input` and `raw_deployed_bytecode` are multi-kilobyte `bytea`
    /// columns nothing downstream reads, so selecting them only costs detoasting and
    /// bandwidth.
    #[test]
    fn source_details_query_leaves_the_raw_bytecodes_in_the_database() {
        assert_eq!(
            source_details_query(&[1, 2])
                .build(DbBackend::Postgres)
                .to_string(),
            concat!(
                r#"SELECT "sources"."id", "sources"."updated_at", CAST("sources"."source_type" AS text), "#,
                r#""sources"."compiler_version", "sources"."compiler_settings", "sources"."file_name", "#,
                r#""sources"."contract_name", "sources"."abi", "sources"."compilation_artifacts", "#,
                r#""sources"."creation_input_artifacts", "sources"."deployed_bytecode_artifacts" "#,
                r#"FROM "sources" WHERE "sources"."id" IN (1, 2)"#,
            ),
        );
    }

    /// Joining the files onto the sources instead would repeat every source row once per
    /// file it has, which in production averages out at north of eleven copies.
    #[test]
    fn source_files_query_does_not_repeat_the_source() {
        assert_eq!(
            source_files_query(&[1, 2])
                .build(DbBackend::Postgres)
                .to_string(),
            concat!(
                r#"SELECT "source_files"."source_id", "files"."name", "files"."content" "#,
                r#"FROM "source_files" INNER JOIN "files" ON "source_files"."file_id" = "files"."id" "#,
                r#"WHERE "source_files"."source_id" IN (1, 2)"#,
            ),
        );
    }
}
