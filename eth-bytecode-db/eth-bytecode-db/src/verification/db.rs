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

macro_rules! insert_then_select {
    ( $txn:expr, $entity_module:ident, $active_model:expr, [ $( ($column:ident, $value:expr) ),+ $(,)?] ) => {
        {
            let result: Result<_, DbErr> = $entity_module::Entity::insert($active_model)
                .on_conflict(OnConflict::new().do_nothing().to_owned())
                .exec($txn)
                .await;

            // Returns the model and the bool flag showing whether the model was actually inserted.
            match result {
                Ok(res) => {
                    let model = $entity_module::Entity::find_by_id(res.last_insert_id)
                        .one($txn)
                        .await
                        .context(format!("select from \"{}\" by \"id\"", stringify!($entity_module)))?
                        .ok_or(anyhow::anyhow!(
                            "select from \"{}\" by \"id\"={} returned no data",
                            stringify!($entity_module),
                            res.last_insert_id
                        ))?;

                    Ok((model, true))
                }
                Err(DbErr::RecordNotInserted) => {
                    let model = $entity_module::Entity::find()
                        $(
                            .filter($entity_module::Column::$column.eq($value))
                        )*
                        .one($txn)
                        .await
                        .context(format!("select from \"{}\" by unique columns", stringify!($entity_module)))?
                        .ok_or(anyhow::anyhow!("select from \"{}\" by unique columns returned no data", stringify!($entity_module)))?;

                    Ok((model, false))
                }
                Err(err) => Err(err).context(format!("insert into \"{}\"", stringify!($entity_module))),
            }
        }
    };
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
    let mut result = Vec::new();
    for (name, content) in files {
        let active_model = files::ActiveModel {
            name: Set(name.clone()),
            content: Set(content.clone()),
            ..Default::default()
        };
        let (file, _inserted) =
            insert_then_select!(txn, files, active_model, [(Name, name), (Content, content)])?;

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
    insert_then_select!(
        txn,
        sources,
        active_model,
        [
            (CompilerVersion, source.compiler_version),
            (CompilerSettings, compiler_settings),
            (FileName, source.file_name),
            (ContractName, source.contract_name),
            (FileIdsHash, file_ids_hash)
        ]
    )
}

async fn insert_source_files(
    txn: &DatabaseTransaction,
    source_model: &sources::Model,
    file_models: &[files::Model],
) -> Result<(), anyhow::Error> {
    let active_models = file_models.iter().map(|file| source_files::ActiveModel {
        source_id: Set(source_model.id),
        file_id: Set(file.id),
        ..Default::default()
    });
    let result = source_files::Entity::insert_many(active_models)
        .on_conflict(OnConflict::new().do_nothing().to_owned())
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
    bytecode_type: BytecodeType,
) -> Result<(), anyhow::Error> {
    let bytecode = {
        let bytecode_type = sea_orm_active_enums::BytecodeType::from(bytecode_type);
        let active_model = bytecodes::ActiveModel {
            source_id: Set(source_id),
            bytecode_type: Set(bytecode_type.clone()),
            ..Default::default()
        };
        let (bytecode, _inserted) = insert_then_select!(
            txn,
            bytecodes,
            active_model,
            [(SourceId, source_id), (BytecodeType, bytecode_type)]
        )?;
        bytecode
    };

    for (order, part) in bytecode_parts.into_iter().enumerate() {
        let part = {
            let part_type = sea_orm_active_enums::PartType::from(&part);
            let active_model = parts::ActiveModel {
                data: Set(part.data().to_vec()),
                part_type: Set(part_type.clone()),
                ..Default::default()
            };
            let (part, _inserted) = insert_then_select!(
                txn,
                parts,
                active_model,
                [(Data, part.data()), (PartType, part_type)]
            )?;
            part
        };

        let bytecode_part = bytecode_parts::ActiveModel {
            bytecode_id: Set(bytecode.id),
            order: Set(order as i64),
            part_id: Set(part.id),
            ..Default::default()
        };
        let result = bytecode_parts::Entity::insert(bytecode_part)
            .on_conflict(OnConflict::new().do_nothing().to_owned())
            .exec(txn)
            .await;
        match result {
            Ok(_) | Err(DbErr::RecordNotInserted) => (),
            Err(err) => return Err(err).context("insert into \"bytecode_parts\""),
        }
    }

    Ok(())
}
