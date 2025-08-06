use crate::{
    clients::blockscout,
    error::ServiceError,
    repository::{chains, interop_messages},
    services::macros::maybe_cache_lookup,
    types::{
        ChainId,
        chains::Chain,
        interop_messages::{ExtendedInteropMessage, MessageDirection},
    },
};
use alloy_primitives::{Address as AddressAlloy, TxHash};
use api_client_framework::HttpApiClient;
use recache::{handler::CacheHandler, stores::redis::RedisStore};
use sea_orm::{DatabaseConnection, prelude::DateTime};
use std::{collections::BTreeMap, sync::Arc};

pub type DecodedCalldataCache = CacheHandler<RedisStore, String, serde_json::Value>;
type BlockscoutClients = BTreeMap<ChainId, Arc<HttpApiClient>>;

pub struct Cluster {
    blockscout_clients: BlockscoutClients,
    decoded_calldata_cache: Option<DecodedCalldataCache>,
}

impl Cluster {
    pub fn new(
        blockscout_clients: BlockscoutClients,
        decoded_calldata_cache: Option<DecodedCalldataCache>,
    ) -> Self {
        Self {
            blockscout_clients,
            decoded_calldata_cache,
        }
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
            let blockscout_client = self
                .blockscout_clients
                .get(&init_chain_id)
                .expect("chain id should be validated")
                .clone();

            let decoded_payload = fetch_decoded_calldata_cached(
                &self.decoded_calldata_cache,
                blockscout_client,
                payload,
                target_address_hash.to_string(),
            )
            .await
            .inspect_err(|e| {
                tracing::error!("failed to fetch decoded calldata: {e}");
            });

            if let Ok(decoded_payload) = decoded_payload {
                message.decoded_payload = Some(decoded_payload);
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

pub async fn fetch_decoded_calldata_cached(
    cache: &Option<DecodedCalldataCache>,
    blockscout_client: Arc<HttpApiClient>,
    calldata: &alloy_primitives::Bytes,
    address_hash: String,
) -> Result<serde_json::Value, ServiceError> {
    let calldata_hash = alloy_primitives::keccak256(calldata).to_string();
    let calldata = calldata.to_string();

    // address_hash is not part of the key, because it does not affect the decoded calldata
    let key = format!("decoded_calldata:{calldata_hash}");

    let get_decoded_payload = || async move {
        blockscout_client
            .request(&blockscout::decode_calldata::DecodeCalldata {
                params: blockscout::decode_calldata::DecodeCalldataParams {
                    calldata,
                    address_hash,
                },
            })
            .await
            .map(|r| r.result)
            .map_err(ServiceError::from)
    };

    maybe_cache_lookup!(cache, key, get_decoded_payload)
}
