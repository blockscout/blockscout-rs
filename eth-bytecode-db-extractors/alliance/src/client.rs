use crate::proxy_verifier;
use anyhow::Context;
use blockscout_display_bytes::Bytes;
use entity::contract_addresses;
use sea_orm::{prelude::Uuid, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::sync::Arc;

#[derive(Clone)]
pub struct Client {
    pub db_client: Arc<DatabaseConnection>,
    pub proxy_verifier_client: proxy_verifier::Client,
}

impl Client {
    pub fn try_new(
        db_client: DatabaseConnection,
        proxy_verifier_url: url::Url,
    ) -> anyhow::Result<Self> {
        let proxy_verifier_client = proxy_verifier::Client::try_new(proxy_verifier_url)?;

        let client = Self {
            db_client: Arc::new(db_client),
            proxy_verifier_client,
        };

        Ok(client)
    }
}

impl Client {
    pub async fn extract(self) -> anyhow::Result<usize> {
        let mut processed = 0;
        while let Some(contract_address_model) = self.next_contract().await? {
            let id = contract_address_model.id;
            let chain_id = contract_address_model.chain_id.to_string();
            let contract_address = Bytes::from(contract_address_model.address.clone());
            let job_id = contract_address_model.job_id;

            tracing::info!(
                contract_address = contract_address.to_string(),
                "processing contract"
            );

            let result = self
                .proxy_verifier_client
                .verify_contract(contract_address_model)
                .await;

            let _response = job_queue::process_result!(
                self.db_client.as_ref(),
                result,
                job_id,
                id = id,
                chain_id = chain_id,
                contract_address = contract_address
            );

            self.mark_as_success(job_id, id, &chain_id, contract_address, None)
                .await?;

            processed += 1;
        }

        Ok(processed)
    }

    async fn mark_as_success(
        &self,
        job_id: Uuid,
        id: i64,
        chain_id: &str,
        contract_address: Bytes,
        message: Option<String>,
    ) -> anyhow::Result<()> {
        job_queue::mark_as_success(self.db_client.as_ref(), job_id, message)
            .await
            .context(format!(
                "saving success details failed for the contract {id} - {chain_id} - {contract_address}",
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
