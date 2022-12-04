// TODO: after implementing https://github.com/blockscout/blockscout-rs/issues/208
// remove this file and use code from "store verification results"

use crate::tests::verifier_mock::{BytecodePart, VerificationResult};
use entity::{
    bytecode_parts, bytecodes, files, parts,
    sea_orm_active_enums::{BytecodeType, PartType, SourceType},
    source_files, sources,
};
use sea_orm::{
    entity::prelude::*,
    sea_query::{Alias, Expr},
    ActiveValue::Set,
    DatabaseTransaction, TransactionTrait,
};

pub async fn insert_verification_result(
    db: &DatabaseConnection,
    verification_result: VerificationResult,
) -> Result<sources::Model, anyhow::Error> {
    let txn = db.begin().await?;

    let raw_creation_input = hex::decode(
        verification_result
            .local_creation_input_parts
            .iter()
            .map(|p| p.data.trim_start_matches("0x"))
            .collect::<Vec<_>>()
            .join(""),
    )
    .unwrap();
    let raw_deployed_bytecode = hex::decode(
        verification_result
            .local_deployed_bytecode_parts
            .iter()
            .map(|p| p.data.trim_start_matches("0x"))
            .collect::<Vec<_>>()
            .join(""),
    )
    .unwrap();

    let compiler_settings = serde_json::from_str(&verification_result.compiler_settings)?;
    let abi = serde_json::from_str(&verification_result.abi.unwrap_or_default())?;

    let source = sources::ActiveModel {
        source_type: Set(SourceType::Solidity),
        compiler_version: Set(verification_result.compiler_version),
        compiler_settings: Set(compiler_settings),
        file_name: Set(verification_result.file_name),
        contract_name: Set(verification_result.contract_name),
        raw_creation_input: Set(raw_creation_input),
        raw_deployed_bytecode: Set(raw_deployed_bytecode),
        abi: Set(Some(abi)),
        ..Default::default()
    }
    .insert(&txn)
    .await?;

    let bytecode = bytecodes::ActiveModel {
        source_id: Set(source.id),
        bytecode_type: Set(BytecodeType::CreationInput),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    insert_parts(
        &txn,
        verification_result.local_creation_input_parts,
        bytecode.id,
    )
    .await?;

    let bytecode = bytecodes::ActiveModel {
        source_id: Set(source.id),
        bytecode_type: Set(BytecodeType::DeployedBytecode),
        ..Default::default()
    }
    .insert(&txn)
    .await?;
    insert_parts(
        &txn,
        verification_result.local_deployed_bytecode_parts,
        bytecode.id,
    )
    .await?;

    for (name, content) in verification_result.sources {
        let file = files::Entity::find()
            .filter(Expr::col(files::Column::Name).eq(name.clone()))
            .filter(Expr::col(files::Column::Content).eq(content.clone()))
            .one(&txn)
            .await?;

        let file = match file {
            Some(file) => file,
            None => {
                files::ActiveModel {
                    name: Set(name),
                    content: Set(content),
                    ..Default::default()
                }
                .insert(&txn)
                .await?
            }
        };
        let _ = source_files::ActiveModel {
            source_id: Set(source.id),
            file_id: Set(file.id),
            ..Default::default()
        }
        .insert(&txn)
        .await;
    }

    txn.commit().await?;
    Ok(source)
}

async fn insert_parts(
    txn: &DatabaseTransaction,
    parts: Vec<BytecodePart>,
    bytecode_id: i64,
) -> Result<(), anyhow::Error> {
    for (order, part) in parts.iter().enumerate() {
        let data = hex::decode(part.data.trim_start_matches("0x")).unwrap();
        let r#type =
            Expr::value(PartType::from(part.r#type.clone())).cast_as(Alias::new("part_type"));
        let part_db = parts::Entity::find()
            .filter(Expr::col(parts::Column::PartType).eq(r#type))
            .filter(Expr::col(parts::Column::Data).eq(data.clone()))
            .one(txn)
            .await?;

        let part_db = match part_db {
            Some(part) => part,
            None => {
                parts::ActiveModel {
                    part_type: Set(PartType::from(part.r#type.clone())),
                    data: Set(data),
                    ..Default::default()
                }
                .insert(txn)
                .await?
            }
        };

        let _ = bytecode_parts::ActiveModel {
            bytecode_id: Set(bytecode_id),
            part_id: Set(part_db.id),
            order: Set(order as i64),
            ..Default::default()
        }
        .insert(txn)
        .await?;
    }

    Ok(())
}
