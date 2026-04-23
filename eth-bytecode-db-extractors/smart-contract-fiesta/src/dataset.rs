use anyhow::Context;
use blockscout_display_bytes::Bytes;
use entity::{contract_addresses, sea_orm_active_enums::VerificationMethod};
use sea_orm::{
    sea_query::OnConflict, ActiveValue::Set, DatabaseConnection, DbErr, EntityTrait,
    TransactionTrait,
};
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
use tokio::fs;

macro_rules! internal_import {
    ( $db:expr, $dataset:expr, $verification_method:expr, $entity_module:ident, $from_data_to_active_model:ident ) => {
        let mut dir = fs::read_dir($dataset).await.context("reading directory")?;
        while let Some(entry) = dir.next_entry().await.context("reading dir entry")? {
            let file_path = entry.path();

            let file = fs::read(file_path.as_path())
                .await
                .context(format!("reading file: {:?}", file_path))?;
            let data: Data = serde_json::from_slice(file.as_slice())
                .context(format!("deserializing file: {:?}", file_path))?;

            let txn = $db.begin().await.context("starting transaction")?;
            match contract_addresses::Entity::insert(contract_addresses::ActiveModel {
                contract_address: Set(data.contract_address.to_vec()),
                verification_method: Set($verification_method),
                ..Default::default()
            })
            .on_conflict(
                OnConflict::column(contract_addresses::Column::ContractAddress)
                    .do_nothing()
                    .to_owned(),
            )
            .exec(&txn)
            .await
            {
                Ok(_) | Err(DbErr::RecordNotInserted) => {}
                err => {
                    err.context(format!(
                        "inserting data into contract_addresses; file: {file_path:?}"
                    ))?;
                }
            };
            let active_model = $from_data_to_active_model(data);
            match $entity_module::Entity::insert(active_model)
                .on_conflict(
                    OnConflict::column($entity_module::Column::ContractAddress)
                        .do_nothing()
                        .to_owned(),
                )
                .exec(&txn)
                .await
            {
                Ok(_) | Err(DbErr::RecordNotInserted) => {}
                Err(err) => Err(err).context(format!(
                    "inserting data into {}; file: {file_path:?}",
                    stringify!($entity_module)
                ))?,
            }
            txn.commit().await.context("committing transaction")?;
        }
    };
}

pub async fn import_dataset(db: Arc<DatabaseConnection>, dataset: PathBuf) -> anyhow::Result<()> {
    let solidity_single_handle = tokio::spawn(import_solidity_single(db.clone(), dataset.clone()));
    let solidity_multiple_handle =
        tokio::spawn(import_solidity_multiple(db.clone(), dataset.clone()));
    let solidity_standard_handle =
        tokio::spawn(import_solidity_standard(db.clone(), dataset.clone()));
    let vyper_single_handle = tokio::spawn(import_vyper_single(db.clone(), dataset.clone()));

    let (
        solidity_single_result,
        solidity_multiple_result,
        solidity_standard_result,
        vyper_single_result,
    ) = tokio::try_join!(
        solidity_single_handle,
        solidity_multiple_handle,
        solidity_standard_handle,
        vyper_single_handle
    )
    .context("import dataset join")?;

    solidity_single_result.context("import solidity single dataset")?;
    solidity_multiple_result.context("import solidity multiple dataset")?;
    solidity_standard_result.context("import solidity standard dataset")?;
    vyper_single_result.context("import vyper single dataset")?;

    Ok(())
}

async fn import_solidity_single(
    db: Arc<DatabaseConnection>,
    mut dataset: PathBuf,
) -> Result<(), anyhow::Error> {
    use entity::solidity_singles;

    #[derive(Clone, Debug, Deserialize)]
    struct Data {
        contract_address: Bytes,
        contract_name: String,
        compiler_version: String,
        optimizations: bool,
        optimization_runs: i64,
        source: String,
    }

    let from_data_to_active_model = |data: Data| {
        let source = data.source.replace(0 as char, "");
        solidity_singles::ActiveModel {
            contract_address: Set(data.contract_address.to_vec()),
            contract_name: Set(data.contract_name),
            compiler_version: Set(data.compiler_version),
            optimizations: Set(data.optimizations),
            optimization_runs: Set(data.optimization_runs),
            source_code: Set(source),
        }
    };

    dataset.push("solidity_single");
    internal_import!(
        db,
        dataset,
        VerificationMethod::SoliditySingle,
        solidity_singles,
        from_data_to_active_model
    );

    Ok(())
}

async fn import_solidity_multiple(
    db: Arc<DatabaseConnection>,
    mut dataset: PathBuf,
) -> Result<(), anyhow::Error> {
    use entity::solidity_multiples;

    #[derive(Clone, Debug, Deserialize)]
    struct Data {
        contract_address: Bytes,
        contract_name: String,
        compiler_version: String,
        optimizations: bool,
        optimization_runs: i64,
        sources: BTreeMap<PathBuf, String>,
    }

    let from_data_to_active_model = |data: Data| {
        let mut sources = data.sources;
        for (_path, source_code) in sources.iter_mut() {
            *source_code = source_code.replace(0 as char, "");
        }
        solidity_multiples::ActiveModel {
            contract_address: Set(data.contract_address.to_vec()),
            contract_name: Set(data.contract_name),
            compiler_version: Set(data.compiler_version),
            optimizations: Set(data.optimizations),
            optimization_runs: Set(data.optimization_runs),
            sources: Set(serde_json::json!(sources)),
        }
    };

    dataset.push("solidity_multiple");
    internal_import!(
        db,
        dataset,
        VerificationMethod::SolidityMultiple,
        solidity_multiples,
        from_data_to_active_model
    );

    Ok(())
}

async fn import_solidity_standard(
    db: Arc<DatabaseConnection>,
    mut dataset: PathBuf,
) -> Result<(), anyhow::Error> {
    use entity::solidity_standards;

    #[derive(Clone, Debug, Deserialize)]
    struct Data {
        contract_address: Bytes,
        contract_name: String,
        compiler_version: String,
        standard_json: serde_json::Value,
    }

    let from_data_to_active_model = |data: Data| solidity_standards::ActiveModel {
        contract_address: Set(data.contract_address.to_vec()),
        contract_name: Set(data.contract_name),
        compiler_version: Set(data.compiler_version),
        standard_json: Set(data.standard_json),
    };

    dataset.push("solidity_standard");
    internal_import!(
        db,
        dataset,
        VerificationMethod::SolidityStandard,
        solidity_standards,
        from_data_to_active_model
    );

    Ok(())
}

async fn import_vyper_single(
    db: Arc<DatabaseConnection>,
    mut dataset: PathBuf,
) -> Result<(), anyhow::Error> {
    use entity::vyper_singles;

    #[derive(Clone, Debug, Deserialize)]
    struct Data {
        contract_address: Bytes,
        contract_name: String,
        compiler_version: String,
        optimizations: bool,
        optimization_runs: i64,
        source: String,
    }

    let from_data_to_active_model = |data: Data| {
        let source = data.source.replace(0 as char, "");
        vyper_singles::ActiveModel {
            contract_address: Set(data.contract_address.to_vec()),
            contract_name: Set(data.contract_name),
            compiler_version: Set(data.compiler_version),
            optimizations: Set(data.optimizations),
            optimization_runs: Set(data.optimization_runs),
            source_code: Set(source),
        }
    };

    dataset.push("vyper_single");
    internal_import!(
        db,
        dataset,
        VerificationMethod::VyperSingle,
        vyper_singles,
        from_data_to_active_model
    );

    Ok(())
}
