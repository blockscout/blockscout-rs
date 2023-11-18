use crate::eth_bytecode_db;
use anyhow::Context;
use blockscout_display_bytes::Bytes;
use entity::contract_addresses;
use eth_bytecode_db_entity::{files, sources};
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2::{
    BytecodeType, Source, VerifySolidityStandardJsonRequest, VerifyVyperStandardJsonRequest,
};
use sea_orm::{
    prelude::Uuid, sea_query::OnConflict, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr,
    EntityTrait, JoinType, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
};
use serde::Serialize;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Serialize)]
struct StandardJson {
    language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    interfaces: Option<serde_json::Value>,
    sources: serde_json::Value,
    settings: serde_json::Value,
}

#[derive(Clone)]
pub struct Client {
    pub db_client: Arc<DatabaseConnection>,
    pub eth_bytecode_db_database_client: Arc<DatabaseConnection>,
    pub eth_bytecode_db_client: eth_bytecode_db::Client,
}

impl Client {
    pub fn try_new(
        db_client: DatabaseConnection,
        eth_bytecode_db_database_client: DatabaseConnection,
        eth_bytecode_db_url: String,
    ) -> anyhow::Result<Self> {
        let eth_bytecode_db_client = eth_bytecode_db::Client::try_new(eth_bytecode_db_url)?;

        let client = Self {
            db_client: Arc::new(db_client),
            eth_bytecode_db_database_client: Arc::new(eth_bytecode_db_database_client),
            eth_bytecode_db_client,
        };

        Ok(client)
    }
}

impl Client {
    pub async fn import_contract_addresses(&self) -> anyhow::Result<usize> {
        let mut processed = 0;
        let mut last_processed_id = 0;
        loop {
            let eth_bytecode_db_sources = sources::Entity::find()
                .filter(sources::Column::Id.gt(last_processed_id))
                .order_by_asc(sources::Column::Id)
                .limit(1000)
                .all(self.eth_bytecode_db_database_client.as_ref())
                .await
                .context("retrieving source ids to process")?;

            let active_models =
                eth_bytecode_db_sources
                    .into_iter()
                    .map(|model| contract_addresses::ActiveModel {
                        source_id: Set(model.id),
                        ..Default::default()
                    });

            processed += active_models.len();
            if processed % 10000 == 0 {
                tracing::info!(
                    processed = processed,
                    last_processed_id = last_processed_id,
                    "Contracts processing"
                );
            }

            match contract_addresses::Entity::insert_many(active_models)
                .on_conflict(OnConflict::new().do_nothing().to_owned())
                .exec(self.db_client.as_ref())
                .await
            {
                Ok(res) => last_processed_id = res.last_insert_id,
                Err(DbErr::RecordNotInserted) => break,
                Err(err) => {
                    return Err(err).context(format!(
                        "inserting contract addresses failed: processed={processed}"
                    ))
                }
            }
        }

        Ok(processed)
    }

    pub async fn verify_contracts(self) -> anyhow::Result<usize> {
        let mut processed = 0;
        while let Some(contract_model) = self.next_contract().await? {
            processed += 1;
            let source_id = contract_model.source_id;
            let job_id = contract_model.job_id;

            tracing::info!(source_id = source_id.to_string(), "contract processed");

            let (source_details_model, source_files) = job_queue::process_result!(
                self.db_client.as_ref(),
                self.import_contract_details(source_id).await,
                job_id,
                source_id = source_id
            );

            let source = job_queue::process_result!(
                self.db_client.as_ref(),
                self.verify_contract(source_details_model, source_files)
                    .await,
                job_id,
                source_id = source_id
            );

            self.mark_as_success(job_id, source).await?;
        }

        Ok(processed)
    }

    async fn import_contract_details(
        &self,
        source_id: i64,
    ) -> anyhow::Result<(sources::Model, BTreeMap<String, String>)> {
        let source_model = sources::Entity::find_by_id(source_id)
            .one(self.eth_bytecode_db_database_client.as_ref())
            .await
            .context("retrieving source details by id")?
            .ok_or(anyhow::anyhow!("source has not been found"))?;

        let files = files::Entity::find()
            .join(JoinType::InnerJoin, files::Relation::SourceFiles.def())
            .filter(eth_bytecode_db_entity::source_files::Column::SourceId.eq(source_id))
            .all(self.eth_bytecode_db_database_client.as_ref())
            .await
            .context("retrieving source files")?
            .into_iter()
            .map(|model| (model.name, model.content))
            .collect();

        Ok((source_model, files))
    }

    async fn verify_contract(
        &self,
        source: sources::Model,
        files: BTreeMap<String, String>,
    ) -> anyhow::Result<Source> {
        let input = Self::generate_input(source.clone(), files);

        let (bytecode, bytecode_type) = (source.raw_creation_input, BytecodeType::CreationInput);

        macro_rules! send_eth_bytecode_db_request {
            ($request_type:tt, $verify:tt) => {{
                let request = $request_type {
                    bytecode: Bytes::from(bytecode).to_string(),
                    bytecode_type: bytecode_type.into(),
                    compiler_version: source.compiler_version,
                    input: serde_json::to_string(&input)
                        .context("serializing standard json input failed")?,
                    metadata: None,
                };

                self.eth_bytecode_db_client.$verify(request).await
            }};
        }

        let source = match source.source_type {
            eth_bytecode_db_entity::sea_orm_active_enums::SourceType::Solidity
            | eth_bytecode_db_entity::sea_orm_active_enums::SourceType::Yul => {
                send_eth_bytecode_db_request!(
                    VerifySolidityStandardJsonRequest,
                    verify_solidity_standard_json
                )
            }
            eth_bytecode_db_entity::sea_orm_active_enums::SourceType::Vyper => {
                send_eth_bytecode_db_request!(
                    VerifyVyperStandardJsonRequest,
                    verify_vyper_standard_json
                )
            }
        }
        .context("verify through eth_bytecode_db failed")?;

        Ok(source)
    }

    fn generate_input(
        source: sources::Model,
        source_files: BTreeMap<String, String>,
    ) -> StandardJson {
        let (language, interfaces) = match source.source_type {
            eth_bytecode_db_entity::sea_orm_active_enums::SourceType::Solidity => {
                ("Solidity", None)
            }
            eth_bytecode_db_entity::sea_orm_active_enums::SourceType::Yul => ("Yul", None),
            eth_bytecode_db_entity::sea_orm_active_enums::SourceType::Vyper => {
                ("Vyper", Some(serde_json::json!({})))
            }
        };

        StandardJson {
            language: language.to_string(),
            sources: Self::generate_sources_input(source_files),
            interfaces,
            settings: source.compiler_settings,
        }
    }

    fn generate_sources_input(files: BTreeMap<String, String>) -> serde_json::Value {
        #[derive(Debug, Serialize)]
        struct Source {
            content: String,
        }

        let sources = files
            .into_iter()
            .map(|(name, content)| (name, Source { content }))
            .collect::<BTreeMap<_, _>>();
        serde_json::to_value(sources).unwrap()
    }

    async fn mark_as_success(&self, job_id: Uuid, source: Source) -> anyhow::Result<()> {
        job_queue::mark_as_success(
            self.db_client.as_ref(),
            job_id,
            Some(
                serde_json::to_string(&source)
                    .context("serializing success result (source) failed")?,
            ),
        )
        .await
        .context(format!("saving success details failed for the contract"))?;

        Ok(())
    }

    async fn next_contract(&self) -> anyhow::Result<Option<contract_addresses::Model>> {
        let next_job_id = job_queue::next_job_id(self.db_client.as_ref())
            .await
            .context("querying the next_job_id")?;

        if let Some(job_id) = next_job_id {
            let model = contract_addresses::Entity::find()
                .filter(contract_addresses::Column::JobId.eq(job_id))
                .one(self.db_client.as_ref())
                .await
                .context("querying contract_address model")?
                .expect("contract_address model does not exist");

            return Ok(Some(model));
        }

        Ok(None)
    }
}
