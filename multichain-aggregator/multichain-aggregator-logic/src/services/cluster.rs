use crate::{
    clients::{
        bens::{get_address, lookup_domain_name},
        blockscout,
        token_info::search_token_infos,
    },
    error::{ParseError, ServiceError},
    repository::{
        address_token_balances::{self, ListAddressTokensPageToken, ListTokenHoldersPageToken},
        addresses, block_ranges, chains, hashes, interop_message_transfers, interop_messages,
        tokens::{self, ListClusterTokensPageToken},
    },
    services::{
        self, MIN_QUERY_LENGTH, dapp_search,
        macros::{maybe_cache_lookup, preload_domain_info},
        quick_search::{self, DomainSearchCache, SearchContext},
    },
    types::{
        ChainId,
        address_token_balances::{AggregatedAddressTokenBalance, TokenHolder},
        addresses::{Address, AggregatedAddressInfo, ChainAddressInfo},
        block_ranges::ChainBlockNumber,
        chains::Chain,
        dapp::MarketplaceDapp,
        domains::{Domain, DomainInfo},
        hashes::{Hash, HashType},
        interop_messages::{ExtendedInteropMessage, MessageDirection},
        search_results::QuickSearchResult,
        token_info::Token,
        tokens::{AggregatedToken, TokenType},
    },
};
use alloy_primitives::{Address as AddressAlloy, TxHash};
use api_client_framework::HttpApiClient;
use bens_proto::blockscout::bens::v1 as bens_proto;
use recache::{handler::CacheHandler, stores::redis::RedisStore};
use regex::Regex;
use sea_orm::{DatabaseConnection, prelude::DateTime};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    str::FromStr,
    sync::{Arc, OnceLock},
};

pub type DecodedCalldataCache = CacheHandler<RedisStore, String, serde_json::Value>;
type BlockscoutClients = BTreeMap<ChainId, Arc<HttpApiClient>>;

pub struct Cluster {
    db: DatabaseConnection,
    chain_ids: HashSet<ChainId>,
    blockscout_clients: BlockscoutClients,
    decoded_calldata_cache: Option<DecodedCalldataCache>,
    quick_search_chains: Vec<ChainId>,
    pub dapp_client: HttpApiClient,
    token_info_client: HttpApiClient,
    bens_client: HttpApiClient,
    bens_protocols: Option<&'static [String]>,
    domain_primary_chain_id: ChainId,
    domain_search_cache: Option<DomainSearchCache>,
    pub marketplace_enabled_cache: services::chains::MarketplaceEnabledCache,
}

impl Cluster {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: DatabaseConnection,
        chain_ids: HashSet<ChainId>,
        blockscout_clients: BlockscoutClients,
        decoded_calldata_cache: Option<DecodedCalldataCache>,
        quick_search_chains: Vec<ChainId>,
        dapp_client: HttpApiClient,
        token_info_client: HttpApiClient,
        bens_client: HttpApiClient,
        bens_protocols: Option<&'static [String]>,
        domain_primary_chain_id: ChainId,
        domain_search_cache: Option<DomainSearchCache>,
    ) -> Self {
        Self {
            db,
            chain_ids,
            blockscout_clients,
            decoded_calldata_cache,
            marketplace_enabled_cache: Default::default(),
            quick_search_chains,
            dapp_client,
            token_info_client,
            bens_client,
            bens_protocols,
            domain_primary_chain_id,
            domain_search_cache,
        }
    }

    pub fn validate_chain_id(&self, chain_id: ChainId) -> Result<(), ServiceError> {
        if !self.chain_ids.contains(&chain_id) {
            return Err(ServiceError::InvalidClusterChainId(chain_id));
        }
        Ok(())
    }

    /// If `chain_ids` is empty, then cluster will include all active chains.
    pub async fn active_chain_ids(&self) -> Result<Vec<ChainId>, ServiceError> {
        let chain_ids = if self.chain_ids.is_empty() {
            services::chains::list_repo_chains_cached(&self.db, true)
                .await?
                .into_iter()
                .map(|c| c.id)
                .collect()
        } else {
            self.chain_ids.iter().cloned().collect()
        };

        Ok(chain_ids)
    }

    pub async fn validate_and_prepare_chain_ids(
        &self,
        chain_ids: Vec<ChainId>,
    ) -> Result<Vec<ChainId>, ServiceError> {
        let active_chain_ids = self.active_chain_ids().await?;

        let chain_ids = if chain_ids.is_empty() {
            active_chain_ids
        } else {
            let active_chain_ids = active_chain_ids.into_iter().collect::<HashSet<_>>();
            let unsupported_chain_ids = chain_ids
                .iter()
                .filter(|chain_id| !active_chain_ids.contains(chain_id))
                .map(|id| id.to_string())
                .collect::<Vec<_>>();

            if !unsupported_chain_ids.is_empty() {
                return Err(ParseError::Custom(format!(
                    "unsupported chain ids provided: {}",
                    unsupported_chain_ids.join(", ")
                ))
                .into());
            }

            chain_ids
        };

        Ok(chain_ids)
    }

    pub async fn list_chains(&self) -> Result<Vec<Chain>, ServiceError> {
        let chain_ids = self.active_chain_ids().await?.into_iter().collect();

        let chains = chains::list_by_ids(&self.db, chain_ids).await?;
        Ok(chains.into_iter().map(|c| c.into()).collect())
    }

    pub async fn get_interop_message(
        &self,
        init_chain_id: ChainId,
        nonce: i64,
    ) -> Result<ExtendedInteropMessage, ServiceError> {
        self.validate_chain_id(init_chain_id)?;

        let message = interop_messages::get(&self.db, init_chain_id, nonce)
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

        let cluster_chain_ids = self.active_chain_ids().await?;

        let (messages, next_page_token) = interop_messages::list(
            &self.db,
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

    pub async fn count_interop_messages(&self, chain_id: ChainId) -> Result<u64, ServiceError> {
        self.validate_chain_id(chain_id)?;

        let cluster_chain_ids = self.active_chain_ids().await?;
        let count = interop_messages::count(&self.db, chain_id, Some(cluster_chain_ids)).await?;
        Ok(count)
    }

    pub async fn get_address_info_aggregated(
        &self,
        address: AddressAlloy,
    ) -> Result<AggregatedAddressInfo, ServiceError> {
        let cluster_chain_ids = self.active_chain_ids().await?;

        let mut address_info = addresses::get_aggregated_address_info(
            &self.db,
            address,
            Some(cluster_chain_ids.clone()),
        )
        .await?
        .unwrap_or_else(|| AggregatedAddressInfo::default(address.into()));

        let (has_tokens, has_interop_message_transfers) = futures::join!(
            address_token_balances::check_if_tokens_at_address(
                &self.db,
                address,
                cluster_chain_ids.clone()
            ),
            interop_message_transfers::check_if_interop_message_transfers_at_address(
                &self.db,
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
        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;
        let res = address_token_balances::list_by_address(
            &self.db,
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
        token_types: Vec<TokenType>,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<ListClusterTokensPageToken>,
    ) -> Result<(Vec<AggregatedToken>, Option<ListClusterTokensPageToken>), ServiceError> {
        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;
        let res =
            tokens::list_aggregated_tokens(&self.db, chain_ids, token_types, page_size, page_token)
                .await?;

        Ok(res)
    }

    pub async fn get_aggregated_token(
        &self,
        address: AddressAlloy,
        chain_id: ChainId,
    ) -> Result<Option<AggregatedToken>, ServiceError> {
        self.validate_chain_id(chain_id)?;
        let token = tokens::get_aggregated_token(&self.db, address, chain_id).await?;

        Ok(token)
    }

    pub async fn list_token_holders(
        &self,
        address: AddressAlloy,
        chain_id: ChainId,
        page_size: u64,
        page_token: Option<ListTokenHoldersPageToken>,
    ) -> Result<(Vec<TokenHolder>, Option<ListTokenHoldersPageToken>), ServiceError> {
        self.validate_chain_id(chain_id)?;
        let holders = address_token_balances::list_token_holders(
            &self.db, address, chain_id, page_size, page_token,
        )
        .await?;

        Ok(holders)
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
            .ok_or_else(|| ServiceError::Internal(anyhow::anyhow!("blockscout client not found")))?
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

    pub async fn search_hashes(
        &self,
        query: String,
        hash_type: Option<HashType>,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<ChainId>,
    ) -> Result<(Vec<Hash>, Option<ChainId>), ServiceError> {
        let hash = match alloy_primitives::B256::from_str(&query) {
            Ok(hash) => hash,
            Err(_) => return Ok((vec![], None)),
        };

        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;

        let (blocks, page_token) =
            hashes::list(&self.db, hash, hash_type, chain_ids, page_size, page_token).await?;

        let hashes = blocks
            .into_iter()
            .map(Hash::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok((hashes, page_token))
    }

    pub async fn search_blocks(
        &self,
        query: String,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<ChainId>,
    ) -> Result<(Vec<Hash>, Option<ChainId>), ServiceError> {
        self.search_hashes(
            query,
            Some(HashType::Block),
            chain_ids,
            page_size,
            page_token,
        )
        .await
    }

    pub async fn search_transactions(
        &self,
        query: String,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<ChainId>,
    ) -> Result<(Vec<Hash>, Option<ChainId>), ServiceError> {
        self.search_hashes(
            query,
            Some(HashType::Transaction),
            chain_ids,
            page_size,
            page_token,
        )
        .await
    }

    pub async fn search_block_numbers(
        &self,
        query: String,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<ChainId>,
    ) -> Result<(Vec<ChainBlockNumber>, Option<ChainId>), ServiceError> {
        let block_number = match alloy_primitives::BlockNumber::from_str(&query) {
            Ok(block_number) => block_number,
            Err(_) => return Ok((vec![], None)),
        };

        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;

        let (block_ranges, page_token) = block_ranges::list_matching_block_ranges_paginated(
            &self.db,
            block_number,
            chain_ids,
            page_size,
            page_token,
        )
        .await?;

        let block_numbers: Vec<_> = block_ranges
            .into_iter()
            .map(|r| ChainBlockNumber {
                chain_id: r.chain_id,
                block_number,
            })
            .collect::<Vec<_>>();

        Ok((block_numbers, page_token))
    }

    pub async fn search_addresses_aggregated(
        &self,
        query: String,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<AddressAlloy>,
    ) -> Result<(Vec<AggregatedAddressInfo>, Option<AddressAlloy>), ServiceError> {
        if query.len() < MIN_QUERY_LENGTH {
            return Ok((vec![], None));
        }

        // TODO: optimize contract name query. Current queries are too slow.
        let (addresses, _contract_name_query) = self.prepare_addresses_query(query).await?;

        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;
        let (mut addresses, page_token) = addresses::list_aggregated_address_infos(
            &self.db,
            addresses,
            Some(chain_ids),
            page_size,
            page_token,
        )
        .await?;

        preload_domain_info!(self, addresses);

        Ok((addresses, page_token))
    }

    pub async fn search_addresses_non_aggregated(
        &self,
        query: String,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<(AddressAlloy, ChainId)>,
    ) -> Result<(Vec<ChainAddressInfo>, Option<(AddressAlloy, ChainId)>), ServiceError> {
        if query.len() < MIN_QUERY_LENGTH {
            return Ok((vec![], None));
        }

        // TODO: optimize contract name query. Current queries are too slow.
        let (addresses, _contract_name_query) = self.prepare_addresses_query(query).await?;

        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;
        let (mut addresses, page_token) = addresses::list_chain_address_infos(
            &self.db,
            addresses,
            Some(chain_ids),
            page_size,
            page_token,
        )
        .await?;

        preload_domain_info!(self, addresses);

        Ok((addresses, page_token))
    }

    async fn prepare_addresses_query(
        &self,
        query: String,
    ) -> Result<(Vec<AddressAlloy>, Option<String>), ServiceError> {
        let (addresses, contract_name_query) = {
            // 1. If query is an address then use it directly
            // 2. If query matches an explicit domain name with TLD (e.g. "name.eth") then
            // lookup the domain name and return the addresses associated with it
            // 3. Otherwise, fallback to a contract name search
            // TODO: support joint paginated search for domain names without TLD and contract names;
            // we need to first handle all pages for domains and then switch to contract names
            if let Ok(address) = alloy_primitives::Address::from_str(&query) {
                (vec![address], None)
            } else if domain_name_with_tld_regex().is_match(&query) {
                let domains = self
                    .search_domains_cached(
                        query.clone(),
                        vec![self.domain_primary_chain_id],
                        1,
                        None,
                    )
                    .await
                    .map(|(d, _)| d)
                    .inspect_err(|err| {
                        tracing::error!(
                            err = ?err,
                            "failed to lookup domains"
                        );
                    })
                    .unwrap_or_default();

                let addresses = domains
                    .iter()
                    .filter_map(|d| d.address)
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();

                if addresses.is_empty() {
                    (vec![], Some(query.to_string()))
                } else {
                    (addresses, None)
                }
            } else {
                (vec![], Some(query.to_string()))
            }
        };

        Ok((addresses, contract_name_query))
    }

    pub async fn search_nfts(
        &self,
        query: String,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<(AddressAlloy, ChainId)>,
    ) -> Result<(Vec<Address>, Option<(AddressAlloy, ChainId)>), ServiceError> {
        let (addresses, contract_name_query) =
            if let Ok(address) = alloy_primitives::Address::from_str(&query) {
                (vec![address], None)
            } else {
                (vec![], Some(query.to_string()))
            };

        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;

        let (addresses, page_token) = addresses::list(
            &self.db,
            addresses,
            contract_name_query,
            chain_ids,
            Some(vec![TokenType::Erc721, TokenType::Erc1155]),
            page_size,
            page_token,
        )
        .await?;

        let addresses = addresses
            .into_iter()
            .map(Address::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        Ok((addresses, page_token))
    }

    pub async fn search_tokens(
        &self,
        query: String,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<String>,
    ) -> Result<(Vec<Token>, Option<String>), ServiceError> {
        if query.len() < MIN_QUERY_LENGTH {
            return Ok((vec![], None));
        }

        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;

        let token_info_search_endpoint = search_token_infos::SearchTokenInfos {
            params: search_token_infos::SearchTokenInfosParams {
                query,
                chain_id: chain_ids,
                page_size: Some(page_size as u32),
                page_token,
            },
        };

        let res = self
            .token_info_client
            .request(&token_info_search_endpoint)
            .await
            .map_err(|err| anyhow::anyhow!("failed to search tokens: {:?}", err))?;

        let mut tokens = res
            .token_infos
            .into_iter()
            .map(|token_info| {
                let mut token = Token::try_from(token_info)?;
                token.icon_url = replace_coingecko_logo_uri_to_large(token.icon_url.as_str());
                Ok(token)
            })
            .collect::<Result<Vec<_>, ParseError>>()?;

        let pks = tokens.iter().map(|t| (&t.address, t.chain_id)).collect();
        let pk_to_address = addresses::get_batch(&self.db, pks).await?;

        for token in tokens.iter_mut() {
            let pk = (token.address, token.chain_id);
            if let Some(address) = pk_to_address.get(&pk) {
                token.is_verified_contract = address.is_verified_contract;
            }
        }

        Ok((tokens, res.next_page_params.map(|p| p.page_token)))
    }

    pub async fn search_domains_cached(
        &self,
        query: String,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<String>,
    ) -> Result<(Vec<Domain>, Option<String>), ServiceError> {
        let primary_chain_id = match chain_ids.first() {
            Some(chain_id) => *chain_id,
            None => return Ok(Default::default()),
        };

        let key = format!(
            "{}:{}:{}:{}:{}",
            query,
            self.bens_protocols.map(|p| p.join(",")).unwrap_or_default(),
            primary_chain_id,
            page_size,
            page_token.clone().unwrap_or_default(),
        );

        let get = || {
            search_domains(
                self.bens_client.clone(),
                query,
                self.bens_protocols,
                primary_chain_id,
                page_size,
                page_token,
            )
        };

        let (domains, next_page_token) =
            maybe_cache_lookup!(self.domain_search_cache.as_ref(), key, get)?;

        Ok((domains, next_page_token))
    }

    pub async fn search_dapps(
        &self,
        query: Option<String>,
        chain_ids: Vec<ChainId>,
        categories: Option<String>,
    ) -> Result<Vec<MarketplaceDapp>, ServiceError> {
        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;

        dapp_search::search_dapps(
            &self.dapp_client,
            query,
            categories,
            chain_ids,
            &self.marketplace_enabled_cache,
        )
        .await
    }

    pub async fn get_domain_info(
        &self,
        addresses: impl IntoIterator<Item = alloy_primitives::Address>,
    ) -> HashMap<alloy_primitives::Address, DomainInfo> {
        let jobs = addresses.into_iter().map(|address| async move {
            let request = bens_proto::GetAddressRequest {
                address: address.to_string(),
                chain_id: self.domain_primary_chain_id,
                protocol_id: None,
            };

            let res = self
                .bens_client
                .request(&get_address::GetAddress { request })
                .await
                .inspect_err(|err| {
                    tracing::error!(
                        error = ?err,
                        address = ?address,
                        "failed to preload domain info"
                    );
                });

            let domain_info = res.map(DomainInfo::try_from).ok()?.ok()?;

            Some((address, domain_info))
        });

        futures::future::join_all(jobs)
            .await
            .into_iter()
            .flatten()
            .collect()
    }

    pub async fn quick_search(
        &self,
        query: String,
        is_aggregated: bool,
    ) -> Result<QuickSearchResult, ServiceError> {
        let context = SearchContext {
            db: Arc::new(self.db.clone()),
            dapp_client: &self.dapp_client,
            token_info_client: &self.token_info_client,
            bens_client: &self.bens_client,
            bens_protocols: self.bens_protocols,
            domain_primary_chain_id: self.domain_primary_chain_id,
            marketplace_enabled_cache: &self.marketplace_enabled_cache,
            domain_search_cache: self.domain_search_cache.as_ref(),
            cluster: self,
            is_aggregated,
        };
        let result = quick_search::quick_search(query, &self.quick_search_chains, &context).await?;
        Ok(result)
    }
}

pub async fn search_domains(
    bens_client: HttpApiClient,
    query: String,
    protocols: Option<&'static [String]>,
    primary_chain_id: ChainId,
    page_size: u64,
    page_token: Option<String>,
) -> Result<(Vec<Domain>, Option<String>), ServiceError> {
    let sort = "registration_date".to_string();
    let order = bens_proto::Order::Desc.into();
    let request = bens_proto::LookupDomainNameRequest {
        name: Some(query),
        chain_id: primary_chain_id,
        only_active: true,
        sort,
        order,
        protocols: protocols.map(|p| p.join(",")),
        page_size: Some(page_size as u32),
        page_token,
    };

    let res = bens_client
        .request(&lookup_domain_name::LookupDomainName { request })
        .await
        .map_err(|err| anyhow::anyhow!("failed to search domains: {:?}", err))?;

    let domains = res
        .items
        .into_iter()
        .map(|d| d.try_into())
        .collect::<Result<Vec<_>, _>>()?;

    let next_page_token = res.next_page_params.map(|p| p.page_token);

    Ok((domains, next_page_token))
}

fn domain_name_with_tld_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[\p{L}\p{N}\p{Emoji}_-]{3,63}\.eth\b").unwrap())
}

fn replace_coingecko_logo_uri_to_large(logo_uri: &str) -> String {
    if logo_uri.starts_with("https://assets.coingecko.com/") {
        logo_uri.replacen("/small/", "/large/", 1)
    } else {
        logo_uri.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_coingecko_logo_uri_to_large() {
        let coingecko_logo = "https://assets.coingecko.com/coins/images/1/small/test_token.png";
        assert_eq!(
            replace_coingecko_logo_uri_to_large(coingecko_logo),
            "https://assets.coingecko.com/coins/images/1/large/test_token.png"
        );

        let other_source_logo = "https://some.other.source.com/coins/images/1/small/test_token.png";
        assert_eq!(
            replace_coingecko_logo_uri_to_large(other_source_logo),
            other_source_logo
        );
    }

    #[test]
    fn test_domain_name_regex() {
        assert!(domain_name_with_tld_regex().is_match("test🙂.eth"));
        assert!(!domain_name_with_tld_regex().is_match("test"));
        assert!(!domain_name_with_tld_regex().is_match("te."));
        assert!(!domain_name_with_tld_regex().is_match("te.eth"));
    }
}
