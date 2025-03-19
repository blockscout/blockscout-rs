use crate::explorer;
use crate::explorer::Explorer;
use anyhow::{anyhow, Context};
use blockscout_display_bytes::ToHex;
use explorer_entity::contracts;
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, Set};
use sea_orm::{DatabaseConnection, EntityTrait};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Clone)]
pub struct Client {
    db: Arc<DatabaseConnection>,
    explorer: Arc<Explorer>,
}

impl Client {
    pub fn new(db: DatabaseConnection, explorer: Explorer) -> Self {
        Self {
            db: Arc::new(db),
            explorer: Arc::new(explorer),
        }
    }

    pub async fn get_source_code(self) -> anyhow::Result<()> {
        while let Some(contract) = self.next_contract().await? {
            let address = contract.address;
            let job_id = contract.job_id;

            tracing::info!(contract_address = address.to_hex(), "processing contract");

            let result = self
                .explorer
                .client
                .request(&explorer::get_source_code::GetSourceCode {
                    address: address.clone(),
                    chain_id: self.explorer.chain_id.clone(),
                    api_key: self.explorer.api_key.clone(),
                })
                .await;

            let response = job_queue::process_result!(
                self.db.as_ref(),
                result,
                job_id,
                address = address.to_hex()
            );

            job_queue::process_result!(
                self.db.as_ref(),
                self.store_source_code(address.clone(), response).await,
                job_id,
                address = address.to_hex()
            );

            self.mark_as_success(job_id, address, None).await?;
        }
        Ok(())
    }

    async fn next_contract(&self) -> anyhow::Result<Option<contracts::Model>> {
        let next_job_id = job_queue::next_job_id(self.db.as_ref())
            .await
            .context("querying the next_job_id")?;

        if let Some(job_id) = next_job_id {
            let model = contracts::Entity::find()
                .filter(contracts::Column::JobId.eq(job_id))
                .one(self.db.as_ref())
                .await
                .context("querying contracts model")?
                .expect(&format!("contracts model does not exist: {job_id}"));

            return Ok(Some(model));
        }

        Ok(None)
    }

    async fn mark_as_success(
        &self,
        job_id: i64,
        address: Vec<u8>,
        message: Option<String>,
    ) -> anyhow::Result<()> {
        job_queue::mark_as_success(self.db.as_ref(), job_id, message)
            .await
            .context(format!(
                "saving success details failed for the contract {}",
                address.to_hex()
            ))?;

        Ok(())
    }

    async fn store_source_code(
        &self,
        address: Vec<u8>,
        response: explorer::get_source_code::GetSourceCodeResponse,
    ) -> anyhow::Result<()> {
        let value = response
            .result
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("result array from response is empty"))?;

        #[derive(Deserialize)]
        struct Result {
            #[serde(rename = "CompilerVersion")]
            compiler_version: String,
        }
        let parsed: Result =
            serde_json::from_value(value.clone()).context("deserializing result")?;

        let is_verified = !parsed.compiler_version.is_empty();
        let updated_model = contracts::ActiveModel {
            address: Set(address),
            is_verified: Set(Some(is_verified)),
            data: Set(Some(value)),
            inserted_at: Default::default(),
            updated_at: Default::default(),
            job_id: Default::default(),
        };
        contracts::Entity::update(updated_model)
            .exec(self.db.as_ref())
            .await
            .context("saving updated contract")?;

        Ok(())
    }
}
