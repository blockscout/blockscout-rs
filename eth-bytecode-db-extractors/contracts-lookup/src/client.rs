use crate::blockscout;
use anyhow::Context;
use blockscout_display_bytes::Bytes;
use entity::contract_addresses;
use sea_orm::{prelude::Uuid, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::Arc;

#[derive(Clone)]
pub struct Client {
    pub db_client: Arc<DatabaseConnection>,
    pub blockscout_client: blockscout::Client,
}

impl Client {
    pub fn try_new(
        db_client: DatabaseConnection,
        blockscout_url: String,
        limit_requests_per_second: u32,
        blockscout_api_key: String,
    ) -> anyhow::Result<Self> {
        let blockscout_client = blockscout::Client::try_new(
            blockscout_url,
            limit_requests_per_second,
            blockscout_api_key,
        )?;

        let client = Self {
            db_client: Arc::new(db_client),
            blockscout_client,
        };

        Ok(client)
    }
}

impl Client {
    pub async fn lookup_contracts(self) -> anyhow::Result<usize> {
        let mut processed = 0;
        while let Some(contract_address_model) = self.next_contract().await? {
            let contract_address = Bytes::from(contract_address_model.contract_address.clone());
            let job_id = contract_address_model.job_id;

            tracing::info!(
                contract_address = contract_address.to_string(),
                "processing contract"
            );

            let response = job_queue::process_result!(
                self.db_client.as_ref(),
                self.blockscout_client
                    .search_contract(contract_address.clone())
                    .await,
                job_id,
                contract_address = contract_address
            );

            self.mark_as_success(job_id, contract_address, response.message)
                .await?;

            processed += 1;
        }

        Ok(processed)
    }

    async fn mark_as_success(
        &self,
        job_id: Uuid,
        contract_address: Bytes,
        message: Option<String>,
    ) -> anyhow::Result<()> {
        job_queue::mark_as_success(self.db_client.as_ref(), job_id, message)
            .await
            .context(format!(
                "saving success details failed for the contract {}",
                contract_address,
            ))?;

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
