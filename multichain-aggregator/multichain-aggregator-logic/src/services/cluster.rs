use crate::{
    error::ServiceError,
    repository::interop_messages,
    types::{
        interop_messages::{InteropMessage, MessageDirection},
        ChainId,
    },
};
use alloy_primitives::{Address as AddressAlloy, TxHash};
use sea_orm::{prelude::DateTime, DatabaseConnection};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Cluster {
    pub name: String,
    pub chain_ids: HashSet<ChainId>,
}

impl Cluster {
    pub fn new(name: String, chain_ids: HashSet<ChainId>) -> Self {
        Self { name, chain_ids }
    }

    pub fn validate_chain_id(&self, chain_id: ChainId) -> Result<(), ServiceError> {
        if !self.chain_ids.contains(&chain_id) {
            return Err(ServiceError::InvalidClusterChainId(chain_id));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list_interop_messages(
        &self,
        db: &DatabaseConnection,
        init_chain_id: Option<ChainId>,
        relay_chain_id: Option<ChainId>,
        address: Option<AddressAlloy>,
        direction: Option<MessageDirection>,
        nonce: Option<i64>,
        page_size: u64,
        page_token: Option<(DateTime, TxHash)>,
    ) -> Result<(Vec<InteropMessage>, Option<(DateTime, TxHash)>), ServiceError> {
        if let Some(init_chain_id) = init_chain_id {
            self.validate_chain_id(init_chain_id)?;
        }
        if let Some(relay_chain_id) = relay_chain_id {
            self.validate_chain_id(relay_chain_id)?;
        }

        let cluster_chain_ids = self.chain_ids.iter().cloned().collect();
        let (interop_messages, next_page_token) = interop_messages::list(
            db,
            init_chain_id,
            relay_chain_id,
            address,
            direction,
            nonce,
            Some(cluster_chain_ids),
            page_size,
            page_token,
        )
        .await?;

        Ok((
            interop_messages
                .into_iter()
                .map(InteropMessage::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            next_page_token,
        ))
    }

    pub async fn count_interop_messages(
        &self,
        db: &DatabaseConnection,
        chain_id: ChainId,
    ) -> Result<u64, ServiceError> {
        self.validate_chain_id(chain_id)?;

        let cluster_chain_ids = self.chain_ids.iter().cloned().collect();
        let count = interop_messages::count(db, chain_id, Some(cluster_chain_ids)).await?;
        Ok(count)
    }
}
