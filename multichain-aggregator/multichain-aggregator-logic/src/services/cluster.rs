use crate::{
    clients::{
        bens::{
            get_address_multichain, get_protocols, lookup_address_multichain,
            lookup_domain_name_multichain,
        },
        blockscout,
    },
    error::{ParseError, ServiceError},
    repository::{
        address_token_balances::{self, ListAddressTokensPageToken, ListTokenHoldersPageToken},
        addresses, block_ranges, chains, hashes, interop_message_transfers, interop_messages,
        tokens::{self, ListClusterTokensPageToken, ListTokenUpdatesPageToken},
    },
    services::{
        self, MIN_QUERY_LENGTH,
        cache::ClusterCaches,
        chain_metrics,
        coin_price::try_fetch_coin_price,
        dapp_search,
        macros::{maybe_cache_lookup, preload_domain_info},
        quick_search::{self, SearchContext, SearchTerm},
    },
    types::{
        ChainId,
        address_token_balances::{AggregatedAddressTokenBalance, TokenHolder},
        addresses::{AggregatedAddressInfo, ChainAddressInfo},
        block_ranges::ChainBlockNumber,
        chain_metrics::{ChainMetricKind, ChainMetrics},
        chains::Chain,
        dapp::MarketplaceDapp,
        domains::{Domain, DomainInfo, ProtocolInfo},
        hashes::{Hash, HashType},
        interop_messages::{ExtendedInteropMessage, MessageDirection},
        portfolio::AddressPortfolio,
        search_results::{QuickSearchResult, Redirect},
        tokens::{AggregatedToken, TokenListUpdate, TokenType},
    },
};
use alloy_primitives::{Address as AddressAlloy, TxHash};
use api_client_framework::HttpApiClient;
use bens_proto::blockscout::bens::v1 as bens_proto;
use itertools::Itertools;
use regex::Regex;
use sea_orm::{
    DatabaseConnection,
    prelude::{BigDecimal, DateTime},
};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap, HashSet},
    str::FromStr,
    sync::{Arc, OnceLock},
};

pub type BlockscoutClients = Arc<BTreeMap<ChainId, Arc<HttpApiClient>>>;

const BENS_PROTOCOLS_LIMIT: usize = 5;

pub struct Cluster {
    db: DatabaseConnection,
    name: String,
    chain_ids: Vec<ChainId>,
    blockscout_clients: BlockscoutClients,
    quick_search_chains: Vec<ChainId>,
    dapp_client: HttpApiClient,
    bens_client: HttpApiClient,
    bens_priority_protocols: Vec<String>,
    caches: ClusterCaches,
}

impl Cluster {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: DatabaseConnection,
        name: String,
        chain_ids: Vec<ChainId>,
        blockscout_clients: BlockscoutClients,
        quick_search_chains: Vec<ChainId>,
        dapp_client: HttpApiClient,
        bens_client: HttpApiClient,
        bens_priority_protocols: Vec<String>,
        caches: ClusterCaches,
    ) -> Self {
        Self {
            db,
            name,
            chain_ids,
            blockscout_clients,
            quick_search_chains,
            dapp_client,
            bens_client,
            bens_priority_protocols,
            caches,
        }
    }

    pub fn validate_chain_id(&self, chain_id: ChainId) -> Result<(), ServiceError> {
        if !self.chain_ids.contains(&chain_id) {
            return Err(ServiceError::InvalidClusterChainId(chain_id));
        }
        Ok(())
    }

    pub fn search_context(&self, is_aggregated: bool) -> SearchContext<'_> {
        SearchContext {
            cluster: self,
            db: Arc::new(self.db.clone()),
            is_aggregated,
        }
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
            self.chain_ids.clone()
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

    pub async fn list_chains(
        &self,
        sort_metric: Option<ChainMetricKind>,
    ) -> Result<Vec<Chain>, ServiceError> {
        let chain_ids = self.active_chain_ids().await?.into_iter().collect();

        let mut chains = chains::list_by_ids(&self.db, chain_ids)
            .await?
            .into_iter()
            .map(|c| c.into())
            .collect::<Vec<Chain>>();

        let metrics = self.list_chain_metrics(sort_metric).await?;
        let order_map = metrics
            .iter()
            .enumerate()
            .map(|(i, m)| (m.chain_id, i))
            .collect::<HashMap<_, _>>();

        chains.sort_by_key(|chain| order_map.get(&chain.id).unwrap_or(&usize::MAX));

        Ok(chains)
    }

    pub async fn list_chain_metrics(
        &self,
        sort_metric: Option<ChainMetricKind>,
    ) -> Result<Vec<ChainMetrics>, ServiceError> {
        let chain_ids = self.active_chain_ids().await?;
        let key = format!("{}:chain_metrics", self.name);

        let blockscout_clients = self.blockscout_clients.clone();
        let get = || async move {
            Ok::<_, ServiceError>(
                chain_metrics::fetch_chain_metrics(&blockscout_clients, &chain_ids).await,
            )
        };

        let mut metrics = maybe_cache_lookup!(self.caches.chain_metrics.as_ref(), key, get)?;

        let sort_metric = sort_metric.unwrap_or_default();
        metrics.sort_by(|left, right| {
            let ordering = match (
                left.metric_value_for_sorting(sort_metric),
                right.metric_value_for_sorting(sort_metric),
            ) {
                (Some(l), Some(r)) => r.total_cmp(&l),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            };

            if ordering == Ordering::Equal {
                left.chain_id.cmp(&right.chain_id)
            } else {
                ordering
            }
        });

        Ok(metrics)
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

        let (has_tokens, has_interop_message_transfers, coin_price, domain_info) = futures::join!(
            address_token_balances::check_if_tokens_at_address(
                &self.db,
                address,
                cluster_chain_ids.clone()
            ),
            interop_message_transfers::check_if_interop_message_transfers_at_address(
                &self.db,
                address,
                cluster_chain_ids,
            ),
            self.fetch_coin_price_cached(),
            self.get_domain_info_cached(address),
        );

        address_info.has_tokens = has_tokens?;
        address_info.has_interop_message_transfers = has_interop_message_transfers?;
        address_info.exchange_rate = coin_price
            .inspect_err(|e| {
                tracing::error!("failed to fetch coin price: {e}");
            })
            .ok()
            .flatten();
        address_info.domain_info = domain_info?;

        Ok(address_info)
    }

    pub async fn get_address_portfolio(
        &self,
        address: AddressAlloy,
        chain_ids: Vec<ChainId>,
    ) -> Result<AddressPortfolio, ServiceError> {
        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;
        let chain_values =
            address_token_balances::portfolio_by_address(&self.db, address, chain_ids).await?;
        let total_value = chain_values
            .iter()
            .fold(BigDecimal::from(0), |acc, v| acc + v.value.clone());

        Ok(AddressPortfolio {
            total_value,
            chain_values,
        })
    }

    pub async fn list_address_tokens(
        &self,
        address: AddressAlloy,
        token_types: Vec<TokenType>,
        chain_ids: Vec<ChainId>,
        query: Option<String>,
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
            query,
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
        query: Option<String>,
        page_size: u64,
        page_token: Option<ListClusterTokensPageToken>,
    ) -> Result<(Vec<AggregatedToken>, Option<ListClusterTokensPageToken>), ServiceError> {
        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;
        let res = tokens::list_aggregated_tokens(
            &self.db,
            vec![],
            chain_ids,
            token_types,
            query,
            page_size,
            page_token,
        )
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

        maybe_cache_lookup!(&self.caches.decoded_calldata, key, get_decoded_payload)
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

        let (addresses, contract_name_query) = self.prepare_addresses_query(query).await?;

        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;
        let (mut addresses, page_token) = addresses::list_aggregated_address_infos(
            &self.db,
            addresses,
            Some(chain_ids),
            contract_name_query,
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

        let (addresses, contract_name_query) = self.prepare_addresses_query(query).await?;

        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;
        let (mut addresses, page_token) = addresses::list_chain_address_infos(
            &self.db,
            addresses,
            Some(chain_ids),
            contract_name_query,
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
            if let Some(address) = SearchTerm::try_parse_address(&query) {
                (vec![address], None)
            } else if domain_name_with_tld_regex().is_match(&query) {
                let domains = self
                    .search_domains_cached(query.clone(), vec![], 1, None)
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

    pub async fn search_nfts_cached(
        &self,
        query: String,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<ListClusterTokensPageToken>,
    ) -> Result<(Vec<AggregatedToken>, Option<ListClusterTokensPageToken>), ServiceError> {
        self.search_tokens_cached(
            query,
            chain_ids,
            vec![TokenType::Erc721, TokenType::Erc1155],
            page_size,
            page_token,
        )
        .await
    }

    pub async fn search_token_infos_cached(
        &self,
        query: String,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<ListClusterTokensPageToken>,
    ) -> Result<(Vec<AggregatedToken>, Option<ListClusterTokensPageToken>), ServiceError> {
        self.search_tokens_cached(
            query,
            chain_ids,
            vec![TokenType::Erc20],
            page_size,
            page_token,
        )
        .await
    }

    pub async fn search_tokens_cached(
        &self,
        query: String,
        chain_ids: Vec<ChainId>,
        token_types: Vec<TokenType>,
        page_size: u64,
        page_token: Option<ListClusterTokensPageToken>,
    ) -> Result<(Vec<AggregatedToken>, Option<ListClusterTokensPageToken>), ServiceError> {
        if query.len() < MIN_QUERY_LENGTH {
            return Ok((vec![], None));
        }

        let (addresses, tokens_query) =
            if let Ok(address) = alloy_primitives::Address::from_str(&query) {
                (vec![address], None)
            } else {
                (vec![], Some(query.to_string()))
            };

        let is_first_page = page_token.is_none();
        let key = {
            let chain_ids_key = chain_ids
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let token_types_key = token_types
                .iter()
                .map(|t| format!("{:?}", t))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{}:{}:{}:{}:{}",
                self.name, query, chain_ids_key, token_types_key, page_size
            )
        };

        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;
        let db = self.db.clone();

        let get = || async move {
            tokens::list_aggregated_tokens(
                &db,
                addresses,
                chain_ids,
                token_types,
                tokens_query,
                page_size,
                page_token,
            )
            .await
            .map_err(ServiceError::from)
        };

        // cache only the first page to speed up quick search
        let (mut tokens, page_token) = if is_first_page {
            maybe_cache_lookup!(self.caches.token_search.as_ref(), key, get)?
        } else {
            get().await?
        };

        tokens.iter_mut().for_each(|token| {
            if let Some(icon_url) = &mut token.icon_url {
                *icon_url = replace_coingecko_logo_uri_to_large(icon_url);
            }
        });

        Ok((tokens, page_token))
    }

    pub async fn fetch_coin_price_cached(&self) -> Result<Option<String>, ServiceError> {
        let chain_ids = self.chain_ids.clone();
        let blockscout_clients = Arc::clone(&self.blockscout_clients);

        let key = format!("{}:coin_price", self.name);
        let get = || async {
            Ok::<_, ServiceError>(try_fetch_coin_price(blockscout_clients, chain_ids).await)
        };
        let coin_price = maybe_cache_lookup!(self.caches.coin_price.as_ref(), key, get)?;

        Ok(coin_price)
    }

    pub async fn search_domains_cached(
        &self,
        query: String,
        _chain_ids: Vec<ChainId>, // NOTE: required for backward compatibility
        page_size: u64,
        page_token: Option<String>,
    ) -> Result<(Vec<Domain>, Option<String>), ServiceError> {
        let protocols = self.prepare_protocol_ids().await?;
        let key = format!(
            "{}:{}:{}:{}",
            query,
            protocols.clone().unwrap_or_default(),
            page_size,
            page_token.clone().unwrap_or_default(),
        );

        let bens_client = self.bens_client.clone();
        let get = || search_domains(bens_client, query, protocols.clone(), page_size, page_token);

        let (domains, next_page_token) =
            maybe_cache_lookup!(self.caches.domain_search.as_ref(), key, get)?;

        Ok((domains, next_page_token))
    }

    // TODO: Add working pagination for dapps
    // Currently this method is just for compatibility with paginated_list_by_query_endpoint! macro
    pub async fn search_dapps_paginated(
        &self,
        query: String,
        chain_ids: Vec<ChainId>,
        _page_size: u64,
        _page_token: Option<String>,
    ) -> Result<(Vec<MarketplaceDapp>, Option<String>), ServiceError> {
        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;

        let dapps = self.search_dapps(Some(query), chain_ids, None).await?;

        Ok((dapps, None))
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
            &self.caches.marketplace_enabled,
        )
        .await
    }

    pub async fn get_domain_info_cached(
        &self,
        address: alloy_primitives::Address,
    ) -> Result<Option<DomainInfo>, ServiceError> {
        let protocols = self.prepare_protocol_ids().await?;
        let key = format!("{}:{}", self.name, address);

        let bens_client = self.bens_client.clone();
        let get = || get_domain_info(bens_client, address, protocols.clone());

        let domain_info = maybe_cache_lookup!(self.caches.domain_info.as_ref(), key, get)?;

        Ok(domain_info)
    }

    pub async fn get_domain_info_batch_cached(
        &self,
        addresses: impl IntoIterator<Item = alloy_primitives::Address>,
    ) -> HashMap<alloy_primitives::Address, DomainInfo> {
        let jobs = addresses.into_iter().map(|address| async move {
            let domain_info = self.get_domain_info_cached(address).await.ok()??;

            Some((address, domain_info))
        });

        futures::future::join_all(jobs)
            .await
            .into_iter()
            .flatten()
            .collect()
    }

    pub async fn get_protocols_cached(&self) -> Result<Vec<ProtocolInfo>, ServiceError> {
        let key = format!("{}:domain_protocols", self.name);
        let bens_client = self.bens_client.clone();
        let chain_ids = self.chain_ids.clone();
        let priority_protocols = self.bens_priority_protocols.clone();
        let get = || get_protocols(bens_client, chain_ids, priority_protocols);
        let protocols = maybe_cache_lookup!(self.caches.domain_protocols.as_ref(), key, get)?;
        Ok(protocols)
    }

    /// Returns comma-separated protocol IDs for use in BENS multichain endpoints
    async fn prepare_protocol_ids(&self) -> Result<Option<String>, ServiceError> {
        let protocols = self.get_protocols_cached().await?;
        if protocols.is_empty() {
            Ok(None)
        } else {
            Ok(Some(
                protocols
                    .iter()
                    .map(|p| p.id.as_str())
                    .take(BENS_PROTOCOLS_LIMIT)
                    .collect::<Vec<_>>()
                    .join(","),
            ))
        }
    }

    pub async fn quick_search(
        &self,
        query: String,
        is_aggregated: bool,
        unlimited_per_chain: bool,
    ) -> Result<QuickSearchResult, ServiceError> {
        let context = self.search_context(is_aggregated);
        let result = quick_search::quick_search(
            query,
            &self.quick_search_chains,
            &context,
            unlimited_per_chain,
        )
        .await?;
        Ok(result)
    }

    pub async fn check_redirect(&self, query: &str) -> Result<Option<Redirect>, ServiceError> {
        let context = self.search_context(false);
        let result = quick_search::check_redirect(query, &context).await?;
        Ok(result)
    }

    pub async fn list_token_updates(
        &self,
        chain_ids: Vec<ChainId>,
        page_size: u64,
        page_token: Option<ListTokenUpdatesPageToken>,
    ) -> Result<(Vec<TokenListUpdate>, Option<ListTokenUpdatesPageToken>), ServiceError> {
        let chain_ids = self.validate_and_prepare_chain_ids(chain_ids).await?;

        let (updates, next_page_token) =
            tokens::list_token_updates(&self.db, chain_ids, page_size, page_token).await?;

        Ok((updates, next_page_token))
    }

    pub async fn lookup_address_domains(
        &self,
        address: String,
        page_size: u32,
        page_token: Option<String>,
    ) -> Result<(Vec<Domain>, Option<String>), ServiceError> {
        let protocols = self.prepare_protocol_ids().await?;
        lookup_address_domains(
            self.bens_client.clone(),
            address,
            protocols,
            page_size,
            page_token,
        )
        .await
    }
}

async fn get_domain_info(
    bens_client: HttpApiClient,
    address: alloy_primitives::Address,
    protocols: Option<String>,
) -> Result<Option<DomainInfo>, ServiceError> {
    let request = bens_proto::GetAddressMultichainRequest {
        address: address.to_string(),
        chain_id: None,
        protocols,
    };

    let res = bens_client
        .request(&get_address_multichain::GetAddressMultichain { request })
        .await
        .inspect_err(|err| {
            tracing::error!(
                error = ?err,
                address = ?address,
                "failed to preload domain info"
            );
        })?;

    let domain_info = DomainInfo::try_from(res).ok();

    Ok(domain_info)
}

async fn get_protocols(
    bens_client: HttpApiClient,
    chain_ids: Vec<ChainId>,
    priority_protocols: Vec<String>,
) -> Result<Vec<ProtocolInfo>, ServiceError> {
    let jobs = chain_ids.into_iter().map(|chain_id| {
        let client = bens_client.clone();
        async move {
            let request = bens_proto::GetProtocolsRequest { chain_id };
            let res = client
                .request(&get_protocols::GetProtocols { request })
                .await
                .inspect_err(
                    |err| tracing::warn!(error = ?err, chain_id = ?chain_id, "failed to fetch protocols for chain"),
                )?;
            Ok::<Vec<ProtocolInfo>, ServiceError>(res.items.into_iter().map(Into::into).collect())
        }
    });

    let results = futures::future::join_all(jobs).await;

    let mut protocols = results
        .into_iter()
        .filter_map(Result::ok)
        .flatten()
        .unique_by(|p| p.id.clone())
        .collect::<Vec<_>>();

    // Protocols in priority list come first, followed by remaining protocols
    // Example:
    // priority_protocols = ["ens", "base"]
    // protocols = ["zns", "base", "ens", "other"]
    // result = ["ens", "base", "zns", "other"]
    if !priority_protocols.is_empty() {
        let priority_index = priority_protocols
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect::<HashMap<_, _>>();
        let fallback = priority_protocols.len();
        protocols.sort_by_key(|p| *priority_index.get(p.id.as_str()).unwrap_or(&fallback));
    }

    Ok(protocols)
}

pub async fn search_domains(
    bens_client: HttpApiClient,
    query: String,
    protocols: Option<String>,
    page_size: u64,
    page_token: Option<String>,
) -> Result<(Vec<Domain>, Option<String>), ServiceError> {
    let sort = "registration_date".to_string();
    let order = bens_proto::Order::Desc.into();
    let chain_id = None;

    let request = bens_proto::LookupDomainNameMultichainRequest {
        name: Some(query),
        chain_id,
        only_active: true,
        sort,
        order,
        protocols,
        page_size: Some(page_size as u32),
        page_token,
    };

    let res = bens_client
        .request(&lookup_domain_name_multichain::LookupDomainNameMultichain { request })
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

pub async fn lookup_address_domains(
    bens_client: HttpApiClient,
    address: String,
    protocols: Option<String>,
    page_size: u32,
    page_token: Option<String>,
) -> Result<(Vec<Domain>, Option<String>), ServiceError> {
    let sort = "registration_date".to_string();
    let order = bens_proto::Order::Desc.into();
    let only_active = true;
    let resolved_to = true;
    let owned_by = true;
    let chain_id = None;

    let request = bens_proto::LookupAddressMultichainRequest {
        address,
        chain_id,
        protocols,
        resolved_to,
        owned_by,
        only_active,
        sort,
        order,
        page_size: Some(page_size),
        page_token,
    };

    let res = bens_client
        .request(&lookup_address_multichain::LookupAddressMultichain { request })
        .await
        .map_err(|err| anyhow::anyhow!("failed to lookup address domains: {:?}", err))?;

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
