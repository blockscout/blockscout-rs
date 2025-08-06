use crate::{
    clients::blockscout,
    error::ServiceError,
    repository::{chains, interop_messages},
    types::{
        ChainId,
        chains::Chain,
        interop_messages::{ExtendedInteropMessage, MessageDirection},
    },
};
use alloy_primitives::{Address as AddressAlloy, TxHash};
use api_client_framework::HttpApiClient;
use sea_orm::{DatabaseConnection, prelude::DateTime};
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct Cluster {
    blockscout_clients: BTreeMap<ChainId, HttpApiClient>,
}

impl Cluster {
    pub fn new(blockscout_clients: BTreeMap<ChainId, HttpApiClient>) -> Self {
        Self { blockscout_clients }
    }

    pub fn validate_chain_id(&self, chain_id: ChainId) -> Result<(), ServiceError> {
        if !self.blockscout_clients.contains_key(&chain_id) {
            return Err(ServiceError::InvalidClusterChainId(chain_id));
        }
        Ok(())
    }

    pub fn chain_ids(&self) -> Vec<ChainId> {
        self.blockscout_clients.keys().cloned().collect()
    }

    pub async fn list_chains(&self, db: &DatabaseConnection) -> Result<Vec<Chain>, ServiceError> {
        let chains = chains::list_by_ids(db, self.chain_ids()).await?;
        Ok(chains.into_iter().map(|c| c.into()).collect())
    }

    pub async fn get_interop_message(
        &self,
        db: &DatabaseConnection,
        init_chain_id: ChainId,
        nonce: i64,
    ) -> Result<ExtendedInteropMessage, ServiceError> {
        self.validate_chain_id(init_chain_id)?;

        let mut message = interop_messages::get(db, init_chain_id, nonce)
            .await?
            .ok_or_else(|| {
                ServiceError::NotFound(format!(
                    "interop message: init_chain_id={init_chain_id}, nonce={nonce}"
                ))
            })?;

        if let (Some(payload), Some(target_address_hash)) =
            (&message.payload, &message.target_address_hash)
        {
            let decoded_payload = self
                .blockscout_clients
                .get(&init_chain_id)
                .expect("chain id was validated")
                .request(&blockscout::decode_calldata::DecodeCalldata {
                    params: blockscout::decode_calldata::DecodeCalldataParams {
                        calldata: payload.to_string(),
                        address_hash: target_address_hash.to_string(),
                    },
                })
                .await
                .inspect_err(|e| {
                    tracing::error!("failed to fetch decoded calldata: {e}");
                });

            if let Ok(decoded_payload) = decoded_payload {
                message.decoded_payload = Some(decoded_payload.result);
            }
        }

        Ok(message)
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
    ) -> Result<(Vec<ExtendedInteropMessage>, Option<(DateTime, TxHash)>), ServiceError> {
        if let Some(init_chain_id) = init_chain_id {
            self.validate_chain_id(init_chain_id)?;
        }
        if let Some(relay_chain_id) = relay_chain_id {
            self.validate_chain_id(relay_chain_id)?;
        }

        let cluster_chain_ids = self.chain_ids();

        let res = interop_messages::list(
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

        Ok(res)
    }

    pub async fn count_interop_messages(
        &self,
        db: &DatabaseConnection,
        chain_id: ChainId,
    ) -> Result<u64, ServiceError> {
        self.validate_chain_id(chain_id)?;

        let cluster_chain_ids = self.chain_ids();
        let count = interop_messages::count(db, chain_id, Some(cluster_chain_ids)).await?;
        Ok(count)
    }
}
