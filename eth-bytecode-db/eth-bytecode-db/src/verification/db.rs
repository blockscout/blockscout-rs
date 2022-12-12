use super::{types, BytecodeType};
use anyhow::Context;
use entity::{
    bytecode_parts, bytecodes, files, parts, sea_orm_active_enums, source_files, sources,
    verified_contracts,
};
use sea_orm::{
    entity::prelude::ColumnTrait, sea_query::OnConflict, ActiveModelTrait, ActiveValue::Set,
    DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, TransactionTrait,
};
use std::collections::BTreeMap;

#[cfg(feature = "test-bins")]
pub async fn _insert_test_data(
    db_client: &DatabaseConnection,
    source_response: types::Source,
) -> Result<i64, anyhow::Error> {
    insert_data(db_client, source_response).await
}

pub(crate) async fn insert_data(
    db_client: &DatabaseConnection,
    source_response: types::Source,
) -> Result<i64, anyhow::Error> {
    let txn = db_client
        .begin()
        .await
        .context("begin database transaction")?;

    let source_files = source_response.source_files.clone();
    let creation_input_parts = source_response.creation_input_parts.clone();
    let deployed_bytecode_parts = source_response.deployed_bytecode_parts.clone();

    let source = insert_source_details(&txn, source_response)
        .await
        .context("insert source details")?;

    insert_source_files(&txn, &source, source_files)
        .await
        .context("insert source files")?;

    insert_bytecodes(
        &txn,
        source.id,
        creation_input_parts,
        types::BytecodeType::CreationInput,
    )
    .await
    .context("insert creation input")?;
    insert_bytecodes(
        &txn,
        source.id,
        deployed_bytecode_parts,
        types::BytecodeType::DeployedBytecode,
    )
    .await
    .context("insert deployed bytecode")?;

    txn.commit().await.context("commit transaction")?;

    Ok(source.id)
}

pub(crate) async fn insert_verified_contract_data(
    db_client: &DatabaseConnection,
    source_id: i64,
    raw_bytecode: Vec<u8>,
    bytecode_type: BytecodeType,
    verification_settings: serde_json::Value,
    verification_type: types::VerificationType,
) -> Result<(), anyhow::Error> {
    verified_contracts::ActiveModel {
        source_id: Set(source_id),
        raw_bytecode: Set(raw_bytecode),
        bytecode_type: Set(sea_orm_active_enums::BytecodeType::from(bytecode_type)),
        verification_settings: Set(verification_settings),
        verification_type: Set(sea_orm_active_enums::VerificationType::from(
            verification_type,
        )),
        ..Default::default()
    }
    .insert(db_client)
    .await
    .context("insert into verified contracts")?;

    Ok(())
}

async fn insert_source_details(
    txn: &DatabaseTransaction,
    source: types::Source,
) -> Result<sources::Model, anyhow::Error> {
    let abi = match source.abi {
        None => None,
        Some(abi) => serde_json::from_str(&abi).context("deserialize abi")?,
    };
    let source = sources::ActiveModel {
        source_type: Set(source.source_type.into()),
        compiler_version: Set(source.compiler_version),
        compiler_settings: Set(serde_json::from_str(&source.compiler_settings)
            .context("deserialize compiler settings")?),
        file_name: Set(source.file_name),
        contract_name: Set(source.contract_name),
        raw_creation_input: Set(source.raw_creation_input),
        raw_deployed_bytecode: Set(source.raw_deployed_bytecode),
        abi: Set(abi),
        ..Default::default()
    }
    .insert(txn)
    .await
    .context("insert into \"sources\"")?;

    Ok(source)
}

async fn insert_source_files(
    txn: &DatabaseTransaction,
    source_model: &sources::Model,
    source_files: BTreeMap<String, String>,
) -> Result<(), anyhow::Error> {
    for (name, content) in source_files {
        let file = {
            let file = files::Entity::find()
                .filter(files::Column::Name.eq(name.clone()))
                .filter(files::Column::Content.eq(content.clone())) // TODO: I believe it is quite expensive to search by the content
                .one(txn)
                .await
                .context("select from \"files\" by \"name\" and \"content\"")?;

            match file {
                Some(file) => file,
                None => files::ActiveModel {
                    name: Set(name),
                    content: Set(content),
                    ..Default::default()
                }
                .insert(txn)
                .await
                .context("insert into \"files\"")?,
            }
        };

        let source_file = source_files::ActiveModel {
            source_id: Set(source_model.id),
            file_id: Set(file.id),
            ..Default::default()
        };
        source_files::Entity::insert(source_file)
            .on_conflict(
                OnConflict::columns([source_files::Column::SourceId, source_files::Column::FileId])
                    .do_nothing()
                    .to_owned(),
            )
            .exec(txn)
            .await
            .context("insert into \"source_files\"")?;
    }

    Ok(())
}

async fn insert_bytecodes(
    txn: &DatabaseTransaction,
    source_id: i64,
    bytecode_parts: Vec<types::BytecodePart>,
    bytecode_type: types::BytecodeType,
) -> Result<(), anyhow::Error> {
    let bytecode = {
        let bytecode_type = sea_orm_active_enums::BytecodeType::from(bytecode_type);
        let bytecode = bytecodes::Entity::find()
            .filter(bytecodes::Column::SourceId.eq(source_id))
            .filter(bytecodes::Column::BytecodeType.eq(bytecode_type.clone()))
            .one(txn)
            .await
            .context("select from \"bytecodes\" by \"source_id\" \"bytecode_type\"")?;

        match bytecode {
            Some(bytecode) => bytecode,
            None => bytecodes::ActiveModel {
                source_id: Set(source_id),
                bytecode_type: Set(bytecode_type),
                ..Default::default()
            }
            .insert(txn)
            .await
            .context("insert into \"bytecodes\"")?,
        }
    };

    for (order, part) in bytecode_parts.into_iter().enumerate() {
        let part = {
            let part_type = sea_orm_active_enums::PartType::from(&part);
            let part_model = parts::Entity::find()
                .filter(parts::Column::Data.eq(part.data()))
                .filter(parts::Column::PartType.eq(part_type.clone()))
                .one(txn)
                .await
                .context("select from \"parts\" by \"data\" and \"part_type\"")?;

            match part_model {
                Some(part_model) => part_model,
                None => parts::ActiveModel {
                    data: Set(part.data_owned()),
                    part_type: Set(part_type),
                    ..Default::default()
                }
                .insert(txn)
                .await
                .context("insert into \"parts\"")?,
            }
        };

        bytecode_parts::ActiveModel {
            bytecode_id: Set(bytecode.id),
            order: Set(order as i64),
            part_id: Set(part.id),
            ..Default::default()
        }
        .insert(txn)
        .await
        .context("insert into \"bytecode_parts\"")?;
    }

    Ok(())
}
