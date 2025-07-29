use crate::{
    clients::{
        bens::{get_address, lookup_domain_name},
        dapp::search_dapps,
        token_info::search_token_infos,
    },
    error::{ParseError, ServiceError},
    repository::{addresses, block_ranges, hashes},
    services::chains,
    types::{
        ChainId,
        addresses::{Address, TokenType},
        block_ranges::ChainBlockNumber,
        dapp::MarketplaceDapp,
        domains::{Domain, DomainInfo},
        hashes::{Hash, HashType},
        search_results::QuickSearchResult,
        token_info::Token,
    },
};
use alloy_primitives::Address as AddressAlloy;
use api_client_framework::HttpApiClient;
use bens_proto::blockscout::bens::v1 as bens_proto;
use blockscout_service_launcher::database::ReadWriteRepo;
use recache::{handler::CacheHandler, stores::redis::RedisStore};
use regex::Regex;
use sea_orm::DatabaseConnection;
use std::{
    collections::HashSet,
    str::FromStr,
    sync::{Arc, OnceLock},
};
use tracing::instrument;

const MIN_QUERY_LENGTH: usize = 3;
const QUICK_SEARCH_NUM_ITEMS: u64 = 50;
const QUICK_SEARCH_ENTITY_LIMIT: usize = 5;

fn domain_name_with_tld_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b[\p{L}\p{N}\p{Emoji}_-]{3,63}\.eth\b").unwrap())
}

macro_rules! maybe_cache_lookup {
    ($cache:expr, $key:expr, $get:expr) => {
        if let Some(cache) = $cache {
            cache
                .default_request()
                .key($key)
                .execute($get)
                .await
                .map_err(|err| err.into())
        } else {
            $get().await
        }
    };
}

pub enum AddressSearchConfig<'a> {
    NFTSearch {
        domain_primary_chain_id: ChainId,
    },
    GeneralSearch {
        bens_protocols: Option<&'a [String]>,
        bens_domain_lookup_limit: u32,
        domain_primary_chain_id: ChainId,
    },
}

impl AddressSearchConfig<'_> {
    pub fn token_types(&self) -> Option<Vec<TokenType>> {
        match self {
            AddressSearchConfig::NFTSearch { .. } => {
                Some(vec![TokenType::Erc721, TokenType::Erc1155])
            }
            AddressSearchConfig::GeneralSearch { .. } => None,
        }
    }

    pub fn domain_primary_chain_id(&self) -> ChainId {
        match self {
            AddressSearchConfig::NFTSearch {
                domain_primary_chain_id,
                ..
            } => *domain_primary_chain_id,
            AddressSearchConfig::GeneralSearch {
                domain_primary_chain_id,
                ..
            } => *domain_primary_chain_id,
        }
    }
}

#[allow(clippy::type_complexity)]
#[instrument(skip_all, level = "info", fields(query = query))]
pub async fn search_addresses(
    db: &DatabaseConnection,
    bens_client: &HttpApiClient,
    config: AddressSearchConfig<'_>,
    query: String,
    chain_ids: Vec<ChainId>,
    page_size: u64,
    page_token: Option<(AddressAlloy, ChainId)>,
) -> Result<(Vec<Address>, Option<(AddressAlloy, ChainId)>), ServiceError> {
    if query.len() < MIN_QUERY_LENGTH {
        return Ok((vec![], None));
    }

    let (addresses, contract_name_query) = match config {
        AddressSearchConfig::GeneralSearch {
            bens_protocols,
            bens_domain_lookup_limit,
            domain_primary_chain_id,
        } => {
            // 1. If query is an address then use it directly
            // 2. If query matches an explicit domain name with TLD (e.g. "name.eth") then
            // lookup the domain name and return the addresses associated with it
            // 3. Otherwise, fallback to a contract name search
            // TODO: support joint paginated search for domain names without TLD and contract names;
            // we need to first handle all pages for domains and then switch to contract names
            if let Ok(address) = alloy_primitives::Address::from_str(&query) {
                (vec![address], None)
            } else if domain_name_with_tld_regex().is_match(&query) {
                let domains = search_domains(
                    bens_client,
                    query.clone(),
                    bens_protocols,
                    domain_primary_chain_id,
                    bens_domain_lookup_limit,
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
        }
        AddressSearchConfig::NFTSearch { .. } => {
            if let Ok(address) = alloy_primitives::Address::from_str(&query) {
                (vec![address], None)
            } else {
                (vec![], Some(query.to_string()))
            }
        }
    };

    let (addresses, page_token) = addresses::list(
        db,
        addresses,
        contract_name_query,
        chain_ids,
        config.token_types(),
        page_size,
        page_token,
    )
    .await?;

    let addresses = addresses
        .into_iter()
        .map(Address::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    let domain_primary_chain_id = config.domain_primary_chain_id();
    let addresses = preload_domain_info(bens_client, domain_primary_chain_id, addresses).await;

    Ok((addresses, page_token))
}

pub async fn preload_domain_info(
    bens_client: &HttpApiClient,
    primary_chain_id: ChainId,
    addresses: impl IntoIterator<Item = Address>,
) -> Vec<Address> {
    let jobs = addresses.into_iter().map(|mut address| async {
        // Preload domain info only for EOA addresses and only for the primary chain instance
        if address.is_contract || address.chain_id != primary_chain_id {
            return address;
        }

        let request = bens_proto::GetAddressRequest {
            address: address.hash.to_string(),
            chain_id: primary_chain_id,
            protocol_id: None,
        };

        let res = bens_client
            .request(&get_address::GetAddress { request })
            .await
            .inspect_err(|err| {
                tracing::error!(
                    error = ?err,
                    address = ?address.hash,
                    "failed to preload domain info"
                );
            });

        if let Ok(res) = res {
            if let Ok(domain_info) = DomainInfo::try_from(res) {
                address.domain_info = Some(domain_info);
            }
        }

        address
    });

    futures::future::join_all(jobs).await
}

pub async fn search_hashes(
    db: &DatabaseConnection,
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

    let (hashes, page_token) =
        hashes::list(db, hash, hash_type, chain_ids, page_size, page_token).await?;

    Ok((
        hashes
            .into_iter()
            .map(Hash::try_from)
            .collect::<Result<Vec<_>, _>>()?,
        page_token,
    ))
}

pub async fn search_block_numbers(
    db: &DatabaseConnection,
    query: String,
    chain_ids: Vec<ChainId>,
    page_size: u64,
    page_token: Option<ChainId>,
) -> Result<(Vec<ChainBlockNumber>, Option<ChainId>), ServiceError> {
    let block_number = match alloy_primitives::BlockNumber::from_str(&query) {
        Ok(block_number) => block_number,
        Err(_) => return Ok((vec![], None)),
    };

    let (block_ranges, page_token) = block_ranges::list_matching_block_ranges_paginated(
        db,
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

pub async fn search_tokens(
    db: &DatabaseConnection,
    token_info_client: &HttpApiClient,
    query: String,
    chain_id: Vec<ChainId>,
    page_size: u64,
    page_token: Option<String>,
) -> Result<(Vec<Token>, Option<String>), ServiceError> {
    if query.len() < MIN_QUERY_LENGTH {
        return Ok((vec![], None));
    }

    let token_info_search_endpoint = search_token_infos::SearchTokenInfos {
        params: search_token_infos::SearchTokenInfosParams {
            query,
            chain_id,
            page_size: Some(page_size as u32),
            page_token,
        },
    };

    let res = token_info_client
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
    let pk_to_address = addresses::get_batch(db, pks).await?;

    for token in tokens.iter_mut() {
        let pk = (token.address, token.chain_id);
        if let Some(address) = pk_to_address.get(&pk) {
            token.is_verified_contract = address.is_verified_contract;
        }
    }

    Ok((tokens, res.next_page_params.map(|p| p.page_token)))
}

pub async fn search_dapps(
    dapp_client: &HttpApiClient,
    query: Option<String>,
    categories: Option<String>,
    chain_ids: Vec<ChainId>,
) -> Result<Vec<MarketplaceDapp>, ServiceError> {
    let res = dapp_client
        .request(&search_dapps::SearchDapps {
            params: search_dapps::SearchDappsParams {
                title: query,
                categories,
                chain_ids,
            },
        })
        .await
        .map_err(|err| anyhow::anyhow!("failed to search dapps: {:?}", err))?;

    let dapps = res
        .into_iter()
        .filter_map(|d| d.try_into().ok())
        .collect::<Vec<_>>();

    Ok(dapps)
}

pub async fn search_domains(
    bens_client: &HttpApiClient,
    query: String,
    protocols: Option<&[String]>,
    primary_chain_id: ChainId,
    page_size: u32,
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
        page_size: Some(page_size),
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

#[allow(clippy::too_many_arguments)]
#[instrument(skip_all, level = "info", fields(query = query))]
pub async fn quick_search(
    query: String,
    priority_chain_ids: &[ChainId],
    search_context: &SearchContext<'_>,
) -> Result<QuickSearchResult, ServiceError> {
    let raw_query = query.trim();

    let terms = parse_search_terms(raw_query);

    // Each search term produces its own `SearchResults` struct.
    // E.g. `SearchTerm::Dapp` job populates only the `dapps` field of its result.
    // We need to merge all of them into a single `SearchResults` struct.
    let jobs = terms.into_iter().map(|t| t.search(search_context));

    let mut results = futures::future::join_all(jobs)
        .await
        .into_iter()
        .fold(QuickSearchResult::default(), |mut acc, r| {
            if let Ok(r) = r {
                acc.merge(r);
            }
            acc
        })
        .filter_and_sort_entities_by_priority(priority_chain_ids);

    results.balance_entities(QUICK_SEARCH_NUM_ITEMS as usize, QUICK_SEARCH_ENTITY_LIMIT);

    Ok(results)
}

pub type UniformChainSearchCache = CacheHandler<RedisStore, String, Vec<Address>>;

pub struct SearchContext<'a> {
    pub db: Arc<ReadWriteRepo>,
    pub dapp_client: &'a HttpApiClient,
    pub token_info_client: &'a HttpApiClient,
    pub bens_client: &'a HttpApiClient,
    pub bens_protocols: Option<&'a [String]>,
    pub domain_primary_chain_id: ChainId,
    pub marketplace_enabled_cache: &'a chains::MarketplaceEnabledCache,
    pub uniform_chain_search_cache: Option<&'a UniformChainSearchCache>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum SearchTerm {
    Hash(alloy_primitives::B256),
    AddressHash(alloy_primitives::Address),
    BlockNumber(alloy_primitives::BlockNumber),
    Dapp(String),
    TokenInfo(String),
    ContractName(String),
    Domain(String),
}

impl SearchTerm {
    #[instrument(skip_all, level = "info", fields(term = ?self), err)]
    async fn search(
        self,
        search_context: &SearchContext<'_>,
    ) -> Result<QuickSearchResult, ServiceError> {
        let mut results = QuickSearchResult::default();

        let db = search_context.db.read_db();

        let num_active_chains = chains::list_repo_chains_cached(db, true).await?.len() as u64;

        match self {
            SearchTerm::Hash(hash) => {
                let (hashes, _) =
                    hashes::list(db, hash, None, vec![], num_active_chains, None).await?;
                let hashes = hashes
                    .into_iter()
                    .map(Hash::try_from)
                    .collect::<Result<Vec<_>, _>>()?;

                let (blocks, transactions): (Vec<_>, Vec<_>) = hashes
                    .into_iter()
                    .partition(|h| h.hash_type == HashType::Block);

                results.blocks.extend(blocks);
                results.transactions.extend(transactions);
            }
            SearchTerm::AddressHash(address) => {
                let (addresses, _) = addresses::list(
                    db,
                    vec![address],
                    None,
                    vec![],
                    None,
                    num_active_chains,
                    None,
                )
                .await?;
                let addresses: Vec<_> = addresses
                    .into_iter()
                    .map(Address::try_from)
                    .collect::<Result<Vec<_>, _>>()?;

                let addresses = preload_domain_info(
                    search_context.bens_client,
                    search_context.domain_primary_chain_id,
                    addresses,
                )
                .await;

                let domains = addresses
                    .iter()
                    .filter_map(|a| a.domain_info.clone())
                    .map(Domain::from)
                    .collect::<Vec<_>>();

                let nfts = addresses
                    .iter()
                    .filter(|a| {
                        matches!(
                            a.token_type,
                            Some(TokenType::Erc721) | Some(TokenType::Erc1155)
                        )
                    })
                    .cloned()
                    .collect::<Vec<_>>();

                results.domains.extend(domains);
                results.addresses.extend(addresses);
                results.nfts.extend(nfts);
            }
            SearchTerm::BlockNumber(block_number) => {
                let (block_ranges, _) = block_ranges::list_matching_block_ranges_paginated(
                    db,
                    block_number,
                    vec![],
                    num_active_chains,
                    None,
                )
                .await?;
                let block_numbers: Vec<_> = block_ranges
                    .into_iter()
                    .map(|r| ChainBlockNumber {
                        chain_id: r.chain_id,
                        block_number,
                    })
                    .collect::<Vec<_>>();

                results.block_numbers.extend(block_numbers);
            }
            SearchTerm::Dapp(query) => {
                let dapp_chains = chains::list_active_chains_cached(
                    db,
                    &[chains::ChainSource::Dapp {
                        dapp_client: search_context.dapp_client,
                    }],
                )
                .await?
                .into_iter()
                .map(|c| c.id)
                .collect::<Vec<_>>();

                let chain_ids = search_context
                    .marketplace_enabled_cache
                    .filter_marketplace_enabled_chains(dapp_chains, |id| *id)
                    .await;

                if !chain_ids.is_empty() {
                    let dapps =
                        search_dapps(search_context.dapp_client, Some(query), None, chain_ids)
                            .await?;

                    results.dapps.extend(dapps);
                }
            }
            SearchTerm::TokenInfo(query) => {
                let (tokens, _) = search_tokens(
                    db,
                    search_context.token_info_client,
                    query,
                    vec![],
                    // TODO: temporary increase number of tokens to improve search quality
                    // until we have a dedicated endpoint for quick search which returns
                    // only one token per chain_id.
                    QUICK_SEARCH_NUM_ITEMS * 2,
                    None,
                )
                .await?;

                results.tokens.extend(tokens);
            }
            SearchTerm::ContractName(query) => {
                let query = query.clone();
                let db = Arc::clone(&search_context.db);

                let get_address = || {
                    let query = query.clone();
                    async move {
                        let addresses = addresses::uniform_chain_search(
                            db.read_db(),
                            query,
                            Some(vec![]),
                            num_active_chains,
                        )
                        .await?
                        .into_iter()
                        .map(Address::try_from)
                        .collect::<Result<Vec<_>, _>>()?;

                        Ok::<_, ServiceError>(addresses)
                    }
                };

                let addresses = maybe_cache_lookup!(
                    search_context.uniform_chain_search_cache,
                    query.clone(),
                    get_address
                )?;

                results.addresses.extend(addresses);
            }
            SearchTerm::Domain(query) => {
                let (domains, _) = search_domains(
                    search_context.bens_client,
                    query,
                    search_context.bens_protocols,
                    search_context.domain_primary_chain_id,
                    QUICK_SEARCH_NUM_ITEMS as u32,
                    None,
                )
                .await?;

                let addresses = domains.iter().filter_map(|d| d.address).collect::<Vec<_>>();
                if !addresses.is_empty() {
                    // Lookup only non-token addresses
                    let (addresses, _) = addresses::list(
                        db,
                        addresses,
                        None,
                        vec![],
                        Some(vec![]),
                        QUICK_SEARCH_NUM_ITEMS,
                        None,
                    )
                    .await?;
                    let addresses: Vec<_> = addresses
                        .into_iter()
                        .map(Address::try_from)
                        .collect::<Result<Vec<_>, _>>()?;

                    let addresses = preload_domain_info(
                        search_context.bens_client,
                        search_context.domain_primary_chain_id,
                        addresses,
                    )
                    .await;

                    results.addresses.extend(addresses);
                }

                results.domains.extend(domains);
            }
        }

        Ok(results)
    }
}

pub fn parse_search_terms(query: &str) -> Vec<SearchTerm> {
    let query = query.trim();
    let mut terms = vec![];

    // If a term is an address or a hash, we can ignore other search types
    if let Ok(hash) = query.parse::<alloy_primitives::B256>() {
        terms.push(SearchTerm::Hash(hash));
        return terms;
    }
    if let Ok(address) = query.parse::<alloy_primitives::Address>() {
        terms.push(SearchTerm::TokenInfo(address.to_string()));
        terms.push(SearchTerm::AddressHash(address));
        return terms;
    }

    if let Ok(block_number) = query.parse::<alloy_primitives::BlockNumber>() {
        terms.push(SearchTerm::BlockNumber(block_number));
    }

    if query.len() >= MIN_QUERY_LENGTH {
        terms.push(SearchTerm::TokenInfo(query.to_string()));
        terms.push(SearchTerm::ContractName(query.to_string()));
        terms.push(SearchTerm::Domain(query.to_string()));
    }

    terms.push(SearchTerm::Dapp(query.to_string()));

    terms
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
    fn test_parse_search_terms() {
        assert_eq!(
            parse_search_terms("0x0000000000000000000000000000000000000000"),
            vec![
                SearchTerm::TokenInfo(alloy_primitives::Address::ZERO.to_string()),
                SearchTerm::AddressHash(alloy_primitives::Address::ZERO),
            ]
        );
        assert_eq!(
            parse_search_terms(
                "0x0000000000000000000000000000000000000000000000000000000000000000"
            ),
            vec![SearchTerm::Hash(alloy_primitives::B256::ZERO)]
        );

        assert_eq!(
            parse_search_terms("0x00"),
            vec![
                SearchTerm::TokenInfo("0x00".to_string()),
                SearchTerm::ContractName("0x00".to_string()),
                SearchTerm::Domain("0x00".to_string()),
                SearchTerm::Dapp("0x00".to_string()),
            ]
        );

        assert_eq!(
            parse_search_terms("1234"),
            vec![
                SearchTerm::BlockNumber(1234),
                SearchTerm::TokenInfo("1234".to_string()),
                SearchTerm::ContractName("1234".to_string()),
                SearchTerm::Domain("1234".to_string()),
                SearchTerm::Dapp("1234".to_string()),
            ]
        );

        assert_eq!(
            parse_search_terms("test.domain"),
            vec![
                SearchTerm::TokenInfo("test.domain".to_string()),
                SearchTerm::ContractName("test.domain".to_string()),
                SearchTerm::Domain("test.domain".to_string()),
                SearchTerm::Dapp("test.domain".to_string()),
            ]
        );
    }

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
