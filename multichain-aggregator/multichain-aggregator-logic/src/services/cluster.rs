use crate::{
    clients::blockscout,
    error::ServiceError,
    repository::{
        address_token_balances::{self, ListAddressTokensPageToken},
        addresses, chains, interop_message_transfers, interop_messages,
        tokens::{self, ListClusterTokensPageToken},
    },
    services::macros::maybe_cache_lookup,
    types::{
        ChainId,
        address_token_balances::AggregatedAddressTokenBalance,
        addresses::AddressInfo,
        chains::Chain,
        interop_messages::{ExtendedInteropMessage, MessageDirection},
        tokens::{AggregatedToken, TokenType},
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

    pub fn validate_and_prepare_chain_ids(
        &self,
        chain_ids: Vec<ChainId>,
    ) -> Result<Vec<ChainId>, ServiceError> {
        let chain_ids = if chain_ids.is_empty() {
            self.chain_ids()
        } else {
            chain_ids
                .iter()
                .map(|c| self.validate_chain_id(*c))
                .collect::<Result<Vec<_>, _>>()?;
            chain_ids
        };

        Ok(chain_ids)
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

        let message = interop_messages::get(db, init_chain_id, nonce)
            .await?
            .ok_or_else(|| {
                ServiceError::NotFound(format!(
                    "interop message: init_chain_id={init_chain_id}, nonce={nonce}"
                ))
            })?;

        let decoded_payload = if let (Some(payload), Some(target_address_hash)) =
            (&message.payload, &message.target_address_hash)
        {
            self.fetch_decoded_calldata_cached(
                payload,
                target_address_hash.to_string(),
                init_chain_id,
            )
            .await
            .inspect_err(|e| {
                tracing::error!("failed to fetch decoded calldata: {e}");
            })
            .ok()
        } else {
            None
        };

        let extended_message = ExtendedInteropMessage {
            message,
            decoded_payload,
        };

        Ok(extended_message)
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

        let (messages, next_page_token) = interop_messages::list(
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

        let messages = messages
            .into_iter()
            .map(|m| ExtendedInteropMessage {
                message: m,
                decoded_payload: None,
            })
            .collect();

        Ok((messages, next_page_token))
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

    pub async fn get_address_info(
        &self,
        db: &DatabaseConnection,
        address: AddressAlloy,
    ) -> Result<AddressInfo, ServiceError> {
        let cluster_chain_ids = self.chain_ids();
        let mut address_info =
            addresses::get_address_info(db, address, Some(cluster_chain_ids.clone()))
                .await?
                .unwrap_or_else(|| AddressInfo::default(address.to_vec()));

        let (has_tokens, has_interop_message_transfers) = futures::join!(
            address_token_balances::check_if_tokens_at_address(
                db,
                address,
                cluster_chain_ids.clone()
            ),
            interop_message_transfers::check_if_interop_message_transfers_at_address(
                db,
                address,
                cluster_chain_ids,
            )
        );

        address_info.has_tokens = has_tokens?;
        address_info.has_interop_message_transfers = has_interop_message_transfers?;

        Ok(address_info)
    }

    pub async fn list_address_tokens(
        &self,
        db: &DatabaseConnection,
        address: AddressAlloy,
        token_types: Vec<TokenType>,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<ListAddressTokensPageToken>,
    ) -> Result<
        (
            Vec<AggregatedAddressTokenBalance>,
            Option<ListAddressTokensPageToken>,
        ),
        ServiceError,
    > {
        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids)?;
        let res = address_token_balances::list_by_address(
            db,
            address,
            token_types,
            chain_ids,
            page_size,
            page_token,
        )
        .await?;

        Ok(res)
    }

    pub async fn list_cluster_tokens(
        &self,
        db: &DatabaseConnection,
        token_types: Vec<TokenType>,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<ListClusterTokensPageToken>,
    ) -> Result<(Vec<AggregatedToken>, Option<ListClusterTokensPageToken>), ServiceError> {
        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids)?;
        let res = tokens::list_aggregated_tokens(db, chain_ids, token_types, page_size, page_token)
            .await?;

        Ok(res)
    }

    pub async fn get_aggregated_token(
        &self,
        db: &DatabaseConnection,
        address: AddressAlloy,
        chain_id: ChainId,
    ) -> Result<Option<AggregatedToken>, ServiceError> {
        self.validate_chain_id(chain_id)?;
        let token = tokens::get_aggregated_token(db, address, chain_id).await?;

        Ok(token)
    }

    async fn fetch_decoded_calldata_cached(
        &self,
        calldata: &alloy_primitives::Bytes,
        address_hash: String,
        chain_id: ChainId,
    ) -> Result<serde_json::Value, ServiceError> {
        let blockscout_client = self
            .blockscout_clients
            .get(&chain_id)
            .expect("chain id should be validated")
            .clone();

        let calldata_hash = alloy_primitives::keccak256(calldata).to_string();
        let calldata = calldata.to_string();

        let key = format!("decoded_calldata:{chain_id}:{address_hash}:{calldata_hash}");

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

        maybe_cache_lookup!(&self.decoded_calldata_cache, key, get_decoded_payload)
    }
}
