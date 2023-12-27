use super::{
    blockscout::{self, BlockscoutClient},
    domain_name::fix_domain_name,
    pagination::{PaginatedList, Paginator},
    schema_selector::subgraph_deployments,
    sql, BatchResolveAddressNamesInput, GetDomainHistoryInput, GetDomainInput, LookupAddressInput,
    LookupDomainInput,
};
use crate::{
    coin_type::coin_name,
    entity::subgraph::{
        domain::{DetailedDomain, Domain},
        domain_event::{DomainEvent, DomainEventTransaction},
    },
    hash_name::{domain_id, hex},
};
use anyhow::Context;
use ethers::types::{Bytes, TxHash, H160};
use sqlx::postgres::PgPool;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    default::Default,
    str::FromStr,
    sync::Arc,
};
use thiserror::Error;
use tracing::instrument;

lazy_static::lazy_static! {
    static ref UNRESOLVABLE_ADDRESSES: Vec<H160> = {
        vec![
            "0x0000000000000000000000000000000000000000",
        ]
        .into_iter()
        .map(|a| H160::from_str(a).unwrap())
        .collect()
    };
}

pub struct SubgraphReader {
    pool: Arc<PgPool>,
    networks: HashMap<i64, Network>,
}

#[derive(Debug, Clone)]
pub struct Network {
    blockscout_client: Arc<BlockscoutClient>,
    subgraphs: Vec<Subgraph>,
    default_subgraph: Subgraph,
}

#[derive(Debug, Clone)]
pub struct Subgraph {
    schema_name: String,
    settings: SubgraphSettings,
}

#[derive(Debug, Clone, Default)]
pub struct SubgraphSettings {
    pub use_cache: bool,
    pub empty_label_hash: Option<Bytes>,
}

#[derive(Debug, Clone)]
pub struct NetworkInfo {
    pub blockscout_client: BlockscoutClient,
    pub subgraph_configs: HashMap<String, SubgraphSettings>,
}

impl NetworkInfo {
    pub fn from_client(blockscout_client: BlockscoutClient) -> Self {
        Self {
            blockscout_client,
            subgraph_configs: Default::default(),
        }
    }
}

impl SubgraphReader {
    pub async fn initialize(
        pool: Arc<PgPool>,
        mut network_infos: HashMap<i64, NetworkInfo>,
    ) -> Result<Self, anyhow::Error> {
        let deployments = subgraph_deployments(&pool).await?;
        tracing::info!(deployments =? deployments, "found subgraph deployments");
        let networks = deployments
            .into_iter()
            .filter(|(_, d)| !d.is_empty())
            .filter_map(|(id, deployments)| {
                let maybe_network = network_infos.remove(&id).map(|info| {
                    let subgraphs: Vec<Subgraph> = deployments
                        .into_iter()
                        .map(|d| {
                            let settings = match info.subgraph_configs.get(&d.subgraph_name) {
                                Some(c) => c.clone(),
                                None => {
                                    tracing::warn!(
                                        "no settings found for subgraph '{}', use default",
                                        d.subgraph_name
                                    );
                                    Default::default()
                                }
                            };
                            Subgraph {
                                schema_name: d.schema_name,
                                settings,
                            }
                        })
                        .collect();
                    let default_subgraph = subgraphs
                        .first()
                        .expect("at least one deployment persist")
                        .to_owned();
                    (
                        id,
                        Network {
                            blockscout_client: Arc::new(info.blockscout_client),
                            subgraphs,
                            default_subgraph,
                        },
                    )
                });
                if maybe_network.is_none() {
                    tracing::warn!("no blockscout url for chain {id}, skip this network")
                }
                maybe_network
            })
            .collect::<HashMap<_, _>>();
        for (id, info) in network_infos.iter() {
            tracing::warn!("no chain found for blockscout url with chain_id {id} and url {}, skip this network", info.blockscout_client.url())
        }
        let this = Self::new(pool, networks);
        this.init_cache().await.context("init cache tables")?;
        tracing::info!(networks =? this.networks.keys().collect::<Vec<_>>(), "initialized subgraph reader");
        Ok(this)
    }

    pub fn new(pool: Arc<PgPool>, networks: HashMap<i64, Network>) -> Self {
        Self { pool, networks }
    }

    pub async fn refresh_cache(&self) -> Result<(), anyhow::Error> {
        for subgraph in self.iter_subgraphs().filter(|s| s.settings.use_cache) {
            let schema = &subgraph.schema_name;
            sql::refresh_address_names_view(self.pool.as_ref(), schema)
                .await
                .context(format!("failed to update {schema}_address_names"))?;
        }
        Ok(())
    }

    pub async fn init_cache(&self) -> Result<(), anyhow::Error> {
        for subgraph in self.iter_subgraphs().filter(|s| s.settings.use_cache) {
            let schema = &subgraph.schema_name;
            sql::create_address_names_view(self.pool.as_ref(), schema)
                .await
                .context(format!(
                    "failed to create address_names view for schema {schema}"
                ))?
        }
        Ok(())
    }

    pub fn iter_subgraphs(&self) -> impl Iterator<Item = &Subgraph> {
        self.networks.values().flat_map(|n| &n.subgraphs)
    }
}

#[derive(Error, Debug)]
pub enum SubgraphReadError {
    #[error("Network with id {0} not found")]
    NetworkNotFound(i64),
    #[error("Db err")]
    DbErr(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

impl SubgraphReader {
    pub async fn get_domain(
        &self,
        input: GetDomainInput,
    ) -> Result<Option<DetailedDomain>, SubgraphReadError> {
        let network = self
            .networks
            .get(&input.network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(input.network_id))?;
        let subgraph = &network.default_subgraph;
        let id = domain_id(&input.name, subgraph.settings.empty_label_hash.clone());
        let domain = sql::get_domain(self.pool.as_ref(), &id, &subgraph.schema_name, &input)
            .await?
            .map(|domain| {
                patch_detailed_domain(
                    self.pool.clone(),
                    &subgraph.schema_name,
                    domain,
                    &input.name,
                    &id,
                )
            });
        Ok(domain)
    }

    pub async fn get_domain_history(
        &self,
        input: GetDomainHistoryInput,
    ) -> Result<Vec<DomainEvent>, SubgraphReadError> {
        let network = self
            .networks
            .get(&input.network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(input.network_id))?;
        let subgraph = &network.default_subgraph;
        let id = domain_id(&input.name, subgraph.settings.empty_label_hash.clone());
        let domain_txns: Vec<DomainEventTransaction> =
            sql::find_transaction_events(self.pool.as_ref(), &subgraph.schema_name, &id, &input)
                .await?;
        let domain_events =
            events_from_transactions(network.blockscout_client.clone(), domain_txns).await?;
        Ok(domain_events)
    }

    pub async fn lookup_domain_name(
        &self,
        input: LookupDomainInput,
    ) -> Result<PaginatedList<Domain>, SubgraphReadError> {
        let network = self
            .networks
            .get(&input.network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(input.network_id))?;
        let subgraph = &network.default_subgraph;
        let id = input
            .name
            .as_ref()
            .map(|name| domain_id(name, subgraph.settings.empty_label_hash.clone()));

        let domains: Vec<Domain> = sql::find_domains(
            self.pool.as_ref(),
            &subgraph.schema_name,
            id.as_deref(),
            &input,
        )
        .await?
        .into_iter()
        .map(|domain| match (&id, &input.name) {
            (Some(id), Some(name)) => {
                patch_domain(self.pool.clone(), &subgraph.schema_name, domain, name, id)
            }
            _ => domain,
        })
        .collect();
        let paginated = input
            .pagination
            .paginate_result(domains)
            .map_err(|e| SubgraphReadError::Internal(format!("cannot paginate result: {e}")))?;
        Ok(paginated)
    }

    pub async fn lookup_address(
        &self,
        input: LookupAddressInput,
    ) -> Result<PaginatedList<Domain>, SubgraphReadError> {
        let network = self
            .networks
            .get(&input.network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(input.network_id))?;
        if UNRESOLVABLE_ADDRESSES.contains(&input.address) {
            return Ok(PaginatedList::empty());
        }
        let domains: Vec<Domain> = sql::find_resolved_addresses(
            self.pool.as_ref(),
            &network.default_subgraph.schema_name,
            &input,
        )
        .await?;
        let paginated = input
            .pagination
            .paginate_result(domains)
            .map_err(|e| SubgraphReadError::Internal(format!("cannot paginate result: {e}")))?;
        Ok(paginated)
    }

    pub async fn batch_resolve_address_names(
        &self,
        input: BatchResolveAddressNamesInput,
    ) -> Result<BTreeMap<String, String>, SubgraphReadError> {
        let network = self
            .networks
            .get(&input.network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(input.network_id))?;
        let subgraph = &network.default_subgraph;
        // remove duplicates
        let addresses: Vec<String> = remove_addresses_from_batch(input.addresses)
            .into_iter()
            .map(hex)
            .collect();
        let addreses_str: Vec<&str> = addresses.iter().map(String::as_str).collect::<Vec<_>>();
        let result = if subgraph.settings.use_cache {
            sql::batch_search_addresses_cached(&self.pool, &subgraph.schema_name, &addreses_str)
                .await?
        } else {
            sql::batch_search_addresses(&self.pool, &subgraph.schema_name, &addreses_str).await?
        };

        let address_to_name: BTreeMap<String, String> = result
            .into_iter()
            .map(|d| (d.resolved_address, d.domain_name))
            .collect();
        tracing::info!(address_to_name =? address_to_name, "{}/{} names found from batch request", address_to_name.len(), addresses.len());
        Ok(address_to_name)
    }
}

fn remove_addresses_from_batch(addresses: impl IntoIterator<Item = H160>) -> Vec<H160> {
    // remove duplicates
    let addresses: HashSet<H160> = addresses
        .into_iter()
        .filter(|a| !UNRESOLVABLE_ADDRESSES.contains(a))
        .collect();
    addresses.into_iter().collect()
}

#[instrument(name = "events_from_transactions", skip_all, fields(job_size = txns.len()), err, level = "info")]
async fn events_from_transactions(
    client: Arc<BlockscoutClient>,
    txns: Vec<DomainEventTransaction>,
) -> Result<Vec<DomainEvent>, SubgraphReadError> {
    let txn_ids: Vec<TxHash> = txns
        .iter()
        .map(|t| TxHash::from_slice(t.transaction_id.as_slice()))
        .collect();
    let mut blockscout_txns = client
        .transactions_batch(txn_ids)
        .await
        .map_err(|e| SubgraphReadError::Internal(e.to_string()))?
        .into_iter()
        .filter_map(|(hash, result)| match result {
            blockscout::Response::Ok(t) => Some((hash, t)),
            e => {
                tracing::warn!(
                    "invalid response from blockscout transaction '{hash:#x}' api: {e:?}"
                );
                None
            }
        })
        .collect::<HashMap<_, _>>();
    let events: Vec<DomainEvent> = txns
        .into_iter()
        .filter_map(|txn| {
            blockscout_txns
                .remove(&TxHash::from_slice(txn.transaction_id.as_slice()))
                .map(|t| DomainEvent {
                    transaction_hash: t.hash,
                    block_number: t.block,
                    timestamp: t.timestamp,
                    from_address: t.from.hash,
                    method: t.method,
                    actions: txn.actions,
                })
        })
        .collect::<Vec<_>>();
    Ok(events)
}

macro_rules! build_fix_domain_name_function {
    ($fn_name:tt, $struct_name:ident) => {
        fn $fn_name(
            pool: Arc<PgPool>,
            schema: &str,
            mut domain: $struct_name,
            input_name: &str,
            input_id: &str,
        ) -> $struct_name {
            if domain.name.as_deref() != Some(input_name) && input_id == domain.id {
                tracing::warn!(
                    domain_id = domain.id,
                    input_name = input_name,
                    domain_name = domain.name,
                    "domain has invalid name, creating task to fix to"
                );
                domain.name = Some(input_name.to_string());
                let input_name = input_name.to_string();
                let input_id = input_id.to_string();
                let schema = schema.to_string();
                tokio::spawn(async move {
                    fix_domain_name(pool, &schema, &input_name, &input_id).await;
                });
            }
            domain
        }
    };
}

build_fix_domain_name_function!(fix_domain_main, Domain);
fn patch_domain(
    pool: Arc<PgPool>,
    schema: &str,
    domain: Domain,
    input_name: &str,
    input_id: &str,
) -> Domain {
    fix_domain_main(pool, schema, domain, input_name, input_id)
}

build_fix_domain_name_function!(fix_detailed_domain_name, DetailedDomain);
fn patch_detailed_domain(
    pool: Arc<PgPool>,
    schema: &str,
    domain: DetailedDomain,
    input_name: &str,
    input_id: &str,
) -> DetailedDomain {
    let mut domain = fix_detailed_domain_name(pool, schema, domain, input_name, input_id);
    domain.other_addresses = sqlx::types::Json(
        domain
            .other_addresses
            .0
            .into_iter()
            .map(|(coin_type, address)| (coin_name(&coin_type), address))
            .collect(),
    );
    domain
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{subgraphs_reader::sql, test_utils::mocked_networks_with_blockscout};
    use ethers::types::Address;
    use pretty_assertions::assert_eq;

    const DEFAULT_CHAIN_ID: i64 = 1;
    const DEFAULT_SCHEMA: &str = "sgd1";

    #[sqlx::test(migrations = "tests/migrations")]
    async fn get_domain_works(pool: PgPool) {
        let pool = Arc::new(pool);
        let networks = mocked_networks_with_blockscout().await;
        let reader = SubgraphReader::initialize(pool.clone(), networks)
            .await
            .expect("failed to init reader");

        // get vitalik domain
        let name = "vitalik.eth".to_string();
        let result = reader
            .get_domain(GetDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name,
                only_active: false,
            })
            .await
            .expect("failed to get vitalik domain")
            .expect("domain not found");
        assert_eq!(result.name.as_deref(), Some("vitalik.eth"));
        assert_eq!(
            result.resolved_address.as_deref(),
            Some("0xd8da6bf26964af9d7eed9e03e53415d37aa96045")
        );
        let other_addresses: HashMap<String, String> = serde_json::from_value(serde_json::json!({
            "ETH": "d8da6bf26964af9d7eed9e03e53415d37aa96045",
            "RSK": "f0d485009714ce586358e3761754929904d76b9d",
        }))
        .unwrap();
        assert_eq!(result.other_addresses, other_addresses.into());

        // get expired domain
        let name = "expired.eth".to_string();
        let result = reader
            .get_domain(GetDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name: name.clone(),
                only_active: false,
            })
            .await
            .expect("failed to get expired domain")
            .expect("expired domain not found");
        assert!(
            result.is_expired,
            "expired domain has is_expired=false: {:?}",
            result
        );
        // since no info in multicoin_addr_changed
        assert!(result.other_addresses.is_empty());

        // get expired domain with only_active filter
        let result = reader
            .get_domain(GetDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name,
                only_active: true,
            })
            .await
            .expect("failed to get expired domain");
        assert!(
            result.is_none(),
            "expired domain returned with only_active=true: {:?}",
            result
        );
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn lookup_domain_name_works(pool: PgPool) {
        let pool = Arc::new(pool);
        let networks = mocked_networks_with_blockscout().await;
        let reader = SubgraphReader::initialize(pool.clone(), networks)
            .await
            .expect("failed to init reader");

        let result = reader
            .lookup_domain_name(LookupDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name: Some("vitalik.eth".to_string()),
                only_active: false,
                pagination: Default::default(),
            })
            .await
            .expect("failed to get vitalik domains");
        assert_eq!(result.next_page_token, None);
        let result = result.items;
        assert_eq!(
            vec![Some("vitalik.eth")],
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
        );
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn lookup_addresses_works(pool: PgPool) {
        let pool = Arc::new(pool);
        let networks = mocked_networks_with_blockscout().await;
        let reader = SubgraphReader::initialize(pool.clone(), networks)
            .await
            .expect("failed to init reader");

        let result = reader
            .lookup_address(LookupAddressInput {
                network_id: DEFAULT_CHAIN_ID,
                address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                resolved_to: true,
                owned_by: false,
                only_active: false,
                pagination: Default::default(),
            })
            .await
            .expect("failed to get vitalik domains");
        assert_eq!(result.next_page_token, None);
        let result = result.items;
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![Some("vitalik.eth"), Some("sashaxyz.eth")]
        );

        let result = reader
            .lookup_address(LookupAddressInput {
                network_id: DEFAULT_CHAIN_ID,
                address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                resolved_to: false,
                owned_by: true,
                only_active: false,
                pagination: Default::default(),
            })
            .await
            .expect("failed to get vitalik domains");
        assert_eq!(result.next_page_token, None);
        let result = result.items;
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![Some("vitalik.eth")]
        );

        // search for expired address
        let result = reader
            .lookup_address(LookupAddressInput {
                network_id: DEFAULT_CHAIN_ID,
                address: addr("0x9f7f7ddbfb8e14d1756580ba8037530da0880b99"),
                resolved_to: true,
                owned_by: true,
                only_active: false,
                pagination: Default::default(),
            })
            .await
            .expect("failed to get expired domains");
        assert_eq!(result.next_page_token, None);
        let result = result.items;
        // expired domain shoudn't be returned as resolved
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![Some("expired.eth")]
        );
        // search for expired address with only_active
        let result = reader
            .lookup_address(LookupAddressInput {
                network_id: DEFAULT_CHAIN_ID,
                address: addr("0x9f7f7ddbfb8e14d1756580ba8037530da0880b99"),
                resolved_to: true,
                owned_by: true,
                only_active: true,
                pagination: Default::default(),
            })
            .await
            .expect("failed to get expired domains");
        assert_eq!(result.next_page_token, None);
        let result = result.items;
        // expired domain shoudn't be returned as resolved
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![]
        );
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn get_domain_history_works(pool: PgPool) {
        let pool = Arc::new(pool);
        let networks = mocked_networks_with_blockscout().await;
        let reader = SubgraphReader::initialize(pool.clone(), networks)
            .await
            .expect("failed to init reader");
        let name = "vitalik.eth".to_string();
        let history = reader
            .get_domain_history(GetDomainHistoryInput {
                network_id: DEFAULT_CHAIN_ID,
                name,
                sort: Default::default(),
                order: Default::default(),
            })
            .await
            .expect("failed to get history");

        let expected_history = vec![
            DomainEvent {
                transaction_hash: tx_hash(
                    "0xdd16deb1ea750037c3ed1cae5ca20ff9db0e664a5146e5a030137d277a9247f3",
                ),
                timestamp: "2017-06-18T08:39:14.000000Z".into(),
                from_address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                method: Some("finalizeAuction".into()),
                actions: vec!["new_owner".into()],
                block_number: 3891899,
            },
            DomainEvent {
                transaction_hash: tx_hash(
                    "0xea30bda97a7e9afcca208d5a648e8ec1e98b245a8884bf589dec8f4aa332fb14",
                ),
                timestamp: "2019-07-10T05:58:51.000000Z".into(),
                from_address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                method: Some("transferRegistrars".into()),
                actions: vec!["new_owner".into()],
                block_number: 8121770,
            },
            DomainEvent {
                transaction_hash: tx_hash(
                    "0x09922ac0caf1efcc8f68ce004f382b46732258870154d8805707a1d4b098dfd0",
                ),
                timestamp: "2019-10-29T13:47:34.000000Z".into(),
                from_address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                method: Some("setAddr".into()),
                actions: vec!["addr_changed".into()],
                block_number: 8834378,
            },
            DomainEvent {
                transaction_hash: tx_hash(
                    "0xc3f86218c67bee8256b74b9b65d746a40bb5318a8b57948b804dbbbc3d0d7864",
                ),
                timestamp: "2020-02-06T18:23:40.000000Z".into(),
                from_address: addr("0x0904dac3347ea47d208f3fd67402d039a3b99859"),
                method: Some("migrateAll".into()),
                actions: vec!["new_owner".into(), "new_resolver".into()],
                block_number: 9430706,
            },
            DomainEvent {
                transaction_hash: tx_hash(
                    "0x160ef4492c731ac6b59beebe1e234890cd55d4c556f8847624a0b47125fe4f84",
                ),
                timestamp: "2021-02-15T17:19:09.000000Z".into(),
                from_address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                method: Some("multicall".into()),
                actions: vec!["addr_changed".into()],
                block_number: 11862656,
            },
            DomainEvent {
                transaction_hash: tx_hash(
                    "0xbb13efab7f1f798f63814a4d184e903e050b38c38aa407f9294079ee7b3110c9",
                ),
                timestamp: "2021-02-15T17:19:17.000000Z".into(),
                from_address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                method: Some("setResolver".into()),
                actions: vec!["new_resolver".into()],
                block_number: 11862657,
            },
        ];
        assert_eq!(expected_history, history);
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn batch_search_works(pool: PgPool) {
        let pool = Arc::new(pool);
        let networks = mocked_networks_with_blockscout().await;
        let reader = SubgraphReader::initialize(pool.clone(), networks)
            .await
            .expect("failed to init reader");

        let addresses = [
            // test.eth
            "0xeefb13c7d42efcc655e528da6d6f7bbcf9a2251d",
            // is was address of test.eth until 13294741 block
            "0x226159d592e2b063810a10ebf6dcbada94ed68b8",
            // vitalik.eth
            "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            // vitalik.eth
            "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            // expired.eth (expired domain)
            "0x9f7f7ddbfb8e14d1756580ba8037530da0880b99",
            // wa🇬🇲i.eth
            "0x9c996076a85b46061d9a70ff81f013853a86b619",
            // not in database
            "0x0000000000000000000000000000000000000000",
            // unresolved domain (labelname is not resolved)
            "0x0101010101010101010101010101010101010101",
        ]
        .into_iter()
        .map(addr)
        .collect();
        let expected_domains = serde_json::from_value(serde_json::json!({
            "0x9c996076a85b46061d9a70ff81f013853a86b619": "wa🇬🇲i.eth",
            "0xd8da6bf26964af9d7eed9e03e53415d37aa96045": "vitalik.eth",
            "0xeefb13c7d42efcc655e528da6d6f7bbcf9a2251d": "test.eth",
        }))
        .unwrap();
        let domains = reader
            .batch_resolve_address_names(BatchResolveAddressNamesInput {
                network_id: DEFAULT_CHAIN_ID,
                addresses,
            })
            .await
            .expect("failed to resolve addresess");
        assert_eq!(domains, expected_domains);
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn fix_domain_name_works(pool: PgPool) {
        let unresolved = "you-dont-know-this-label.eth";
        let pool = Arc::new(pool);
        let networks = mocked_networks_with_blockscout().await;
        let reader = SubgraphReader::initialize(pool.clone(), networks)
            .await
            .expect("failed to init reader");

        // Make sure that database contains unresolved domain
        let domain = sql::get_domain(
            pool.as_ref(),
            &domain_id(unresolved, None),
            DEFAULT_SCHEMA,
            &GetDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name: unresolved.to_string(),
                only_active: false,
            },
        )
        .await
        .expect("failed to get domain")
        .expect("unresolved domain not found using sql");
        assert_eq!(
            domain.name.as_deref(),
            Some("[0b0e081f36b3970ff8e337f0ff7bdfad321a702fa00916b6ccfc47877144f7ad].eth")
        );

        // After reader requests domain should be resolved
        let domain = reader
            .get_domain(GetDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name: unresolved.to_string(),
                only_active: false,
            })
            .await
            .expect("failed to get domain")
            .expect("unresolved domain not found using reader");
        assert_eq!(domain.name.as_deref(), Some(unresolved));

        // Make sure that unresolved name in database became resolved
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let domain = sql::get_domain(
            pool.as_ref(),
            &domain_id(unresolved, None),
            DEFAULT_SCHEMA,
            &GetDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name: unresolved.to_string(),
                only_active: false,
            },
        )
        .await
        .expect("failed to get domain")
        .expect("unresolved domain not found using sql");
        assert_eq!(domain.name.as_deref(), Some(unresolved));
    }

    fn addr(a: &str) -> Address {
        let a = a.trim_start_matches("0x");
        Address::from_slice(
            hex::decode(a)
                .expect("invalid hex provided in addr()")
                .as_slice(),
        )
    }

    fn tx_hash(h: &str) -> TxHash {
        let h = h.trim_start_matches("0x");
        TxHash::from_slice(
            hex::decode(h)
                .expect("invalid hex provided in tx_hash()")
                .as_slice(),
        )
    }
}
