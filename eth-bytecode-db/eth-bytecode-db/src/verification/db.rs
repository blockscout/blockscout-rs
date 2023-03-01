use super::{types, BytecodeType};
use anyhow::Context;
use entity::{
    bytecode_parts, bytecodes, files, parts, sea_orm_active_enums, source_files, sources,
    verified_contracts,
};
use sea_orm::{
    entity::prelude::ColumnTrait,
    prelude::{Json, Uuid},
    sea_query::OnConflict,
    ActiveModelTrait,
    ActiveValue::Set,
    ConnectionTrait, DatabaseBackend, DatabaseConnection, DatabaseTransaction, DbErr, EntityTrait,
    QueryFilter, Statement, TransactionTrait,
};
use std::collections::{BTreeMap, BTreeSet};

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

    let files = insert_files(&txn, source_files.clone())
        .await
        .context("insert files")?;
    let (source, inserted) = insert_source_details(&txn, source_response, files.as_ref())
        .await
        .context("insert source details")?;

    // If no new source has been inserted, no new source_files or bytecodes to be added.
    if inserted {
        insert_source_files(&txn, &source, files.as_ref())
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
    }

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

async fn insert_files(
    txn: &DatabaseTransaction,
    files: BTreeMap<String, String>,
) -> Result<Vec<files::Model>, anyhow::Error> {
    // Insert non-existed files
    {
        let active_models = files.iter().map(|(name, content)| files::ActiveModel {
            name: Set(name.clone()),
            content: Set(content.clone()),
            ..Default::default()
        });
        match files::Entity::insert_many(active_models)
            .on_conflict(
                OnConflict::columns([files::Column::Name, files::Column::Content])
                    .do_nothing()
                    .to_owned(),
            )
            .exec(txn)
            .await
        {
            Ok(_) | Err(DbErr::RecordNotInserted) => (),
            Err(err) => return Err(err).context("insert into \"files\""),
        }
    }

    let mut result = Vec::new();
    for (name, content) in files {
        let file = files::Entity::find()
            .filter(files::Column::Name.eq(name.clone()))
            .filter(files::Column::Content.eq(content.clone())) // TODO: Is it expensive to search by the content?
            .one(txn)
            .await
            .context("select from \"files\" by \"name\" and \"content\"")?
            .ok_or(anyhow::anyhow!("select from \"files\" by \"name={name}\" and \"content\"={content} returned no data"))?;
        result.push(file);
    }

    Ok(result)
}

async fn insert_source_details(
    txn: &DatabaseTransaction,
    source: types::Source,
    file_models: &[files::Model],
) -> Result<(sources::Model, bool), anyhow::Error> {
    let abi = source
        .abi
        .map(|abi| serde_json::from_str(&abi).context("deserialize abi"))
        .transpose()?;

    // To ensure uniqueness and ordering properties
    let file_ids: BTreeSet<_> = file_models.iter().map(|file| file.id).collect();

    // Results in `SELECT md5({1,2,3}::text)
    let file_ids_hash_query = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        "SELECT md5($1::text)::uuid",
        [sea_orm::Value::from(
            file_ids.into_iter().collect::<Vec<_>>(),
        )],
    );
    let file_ids_hash: Uuid = txn
        .query_one(file_ids_hash_query)
        .await
        .context("calculate hash of file ids")?
        .ok_or(anyhow::anyhow!(
            "selection of file ids resulted in empty result"
        ))
        .context("calculate hash of file ids")?
        .try_get("", "md5")
        .context("calculate hash of file ids")?;

    let compiler_settings: Json =
        serde_json::from_str(&source.compiler_settings).context("deserialize compiler settings")?;

    {
        let active_model = sources::ActiveModel {
            source_type: Set(source.source_type.into()),
            compiler_version: Set(source.compiler_version.clone()),
            compiler_settings: Set(compiler_settings.clone()),
            file_name: Set(source.file_name.clone()),
            contract_name: Set(source.contract_name.clone()),
            raw_creation_input: Set(source.raw_creation_input.clone()),
            raw_deployed_bytecode: Set(source.raw_deployed_bytecode.clone()),
            abi: Set(abi.clone()),
            file_ids_hash: Set(file_ids_hash),
            ..Default::default()
        };
        let result = sources::Entity::insert(active_model)
            .on_conflict(
                OnConflict::columns([
                    sources::Column::CompilerVersion,
                    // sources::Column::CompilerSettings, sources::Column::FileName, sources::Column::ContractName, sources::Column::FileIdsHash
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(txn)
            .await;

        let (source, inserted) = match result {
            Ok(res) => {
                let source = sources::Entity::find_by_id(res.last_insert_id)
                    .one(txn)
                    .await
                    .context("select from \"sources\" by \"id\"")?
                    .ok_or(anyhow::anyhow!(
                        "select from \"sources\" by \"id\"={} returned no data",
                        res.last_insert_id
                    ))?;

                (source, true)
            }
            Err(DbErr::RecordNotInserted) => {
                let source = sources::Entity::find()
                    .filter(sources::Column::CompilerVersion.eq(source.compiler_version.clone()))
                    .filter(sources::Column::CompilerSettings.eq(compiler_settings.clone()))
                    .filter(sources::Column::FileName.eq(source.file_name.clone()))
                    .filter(sources::Column::ContractName.eq(source.contract_name.clone()))
                    .filter(sources::Column::FileIdsHash.eq(file_ids_hash))
                    .one(txn)
                    .await
                    .context("select from \"sources\" by \"compiler_version\", \"compiler_settings\", \"file_name\", \"contract_name\", and \"file_ids_hash\"")?
                    .ok_or(anyhow::anyhow!("select from \"sources\" by \"compiler_version\", \"compiler_settings\", \"file_name\", \"contract_name\", and \"file_ids_hash\" returned no data"))?;

                (source, false)
            }
            Err(err) => return Err(err).context("insert into \"sources\""),
        };

        Ok((source, inserted))
    }
}

async fn insert_source_files(
    txn: &DatabaseTransaction,
    source_model: &sources::Model,
    file_models: &[files::Model],
) -> Result<(), anyhow::Error> {
    let active_models = file_models
        .iter()
        .map(|file| source_files::ActiveModel {
            source_id: Set(source_model.id),
            file_id: Set(file.id),
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let result = source_files::Entity::insert_many(active_models)
        .on_conflict(
            OnConflict::columns([source_files::Column::SourceId, source_files::Column::FileId])
                .do_nothing()
                .to_owned(),
        )
        .exec(txn)
        .await;
    match result {
        Ok(_) | Err(DbErr::RecordNotInserted) => (),
        Err(err) => return Err(err).context("insert into \"source_files\""),
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

        let bytecode_part = bytecode_parts::ActiveModel {
            bytecode_id: Set(bytecode.id),
            order: Set(order as i64),
            part_id: Set(part.id),
            ..Default::default()
        };
        let result = bytecode_parts::Entity::insert(bytecode_part)
            .on_conflict(
                OnConflict::columns([
                    bytecode_parts::Column::BytecodeId,
                    bytecode_parts::Column::Order,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(txn)
            .await;
        match result {
            Ok(_) | Err(DbErr::RecordNotInserted) => (),
            Err(err) => return Err(err).context("insert into \"bytecode_parts\""),
        }
    }

    Ok(())
}
