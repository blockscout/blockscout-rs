use ethabi::Address;
use ethers::{
    providers::{Middleware, Provider},
    types::{Filter, Log},
};

use super::common_transport::CommonTransport;
use anyhow::Result;

pub struct EthProvider {
    provider: Provider<CommonTransport>,
}

impl EthProvider {
    pub async fn new(rpc_url: &str) -> Result<Self> {
        let transport = CommonTransport::new(rpc_url.to_string()).await?;
        let provider = Provider::new(transport);
        Ok(Self { provider })
    }

    pub async fn get_block_number(&self) -> Result<u64> {
        Ok(self.provider.get_block_number().await?.as_u64())
    }

    /// Fetches event from the blockchain in batches.
    /// `soft_limit` allows to stop fetching logs if the limit is reached,
    /// but the actual number of logs might be greater than the limit
    pub async fn get_logs(
        &self,
        address: &str,
        event: &str,
        from: u64,
        to: u64,
        batch_size: u64,
        soft_limit: Option<u64>,
    ) -> Result<Vec<Log>> {
        if from > to {
            return Ok(vec![]);
        }

        let mut temp_from = from;
        let mut temp_to = to.min(from + batch_size);
        let mut logs = vec![];
        loop {
            let filter = Filter::new()
                .address(address.parse::<Address>()?)
                .event(event)
                .from_block(temp_from)
                .to_block(temp_to);

            logs.append(&mut self.provider.get_logs(&filter).await?);
            tracing::info!(
                "fetched {} logs for event '{}' from block {} to block {}",
                logs.len(),
                event,
                temp_from,
                temp_to
            );

            if let Some(limit) = soft_limit {
                if logs.len() as u64 >= limit {
                    break;
                }
            }

            temp_from = temp_to + 1;
            temp_to = to.min(temp_from + batch_size);

            if temp_from > to {
                break;
            }
        }

        Ok(logs)
    }
}
