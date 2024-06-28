use super::{
    domain_tokens::extract_tokens_from_domain,
    pagination::{PaginatedList, Paginator},
    patch::{patch_detailed_domain, patch_domain},
    sql,
    types::*,
};
use crate::{
    blockscout,
    blockscout::BlockscoutClient,
    entity::subgraph::{
        domain::{DetailedDomain, Domain},
        domain_event::{DomainEvent, DomainEventTransaction},
    },
    protocols::{
        AddressResolveTechnique, DeployedProtocol, Network, Protocol, ProtocolError, ProtocolInfo,
        Protocoler,
    },
    subgraphs_reader::{
        resolve_addresses::resolve_addresses,
        sql::{CachedView, DbErr},
    },
};
use anyhow::{anyhow, Context};
use ethers::types::{Address, TxHash};
use nonempty::{nonempty, NonEmpty};
use sqlx::postgres::PgPool;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};
use thiserror::Error;
use tracing::instrument;

lazy_static::lazy_static! {
    static ref UNRESOLVABLE_ADDRESSES_PREFIXES: Vec<&'static str> = {
        // if first 16 bytes are zeros, this is precompiled contract
        vec![
            "0x00000000000000000000000000000000"
        ]
    };
}

const MAX_RESOLVE_ADDRESSES: usize = 100;

#[derive(Error, Debug)]
pub enum SubgraphReadError {
    #[error("failed to get protocol info: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("Db err")]
    DbErr(#[from] DbErr),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

pub struct SubgraphReader {
    pool: Arc<PgPool>,
    protocoler: Protocoler,
}

impl SubgraphReader {
    #[instrument(name = "SubgraphReader::initialize", skip_all, err, level = "info")]
    pub async fn initialize(
        pool: Arc<PgPool>,
        networks: HashMap<i64, Network>,
        protocol_infos: HashMap<String, ProtocolInfo>,
    ) -> Result<Self, anyhow::Error> {
        let deployments = sql::get_deployments(&pool)
            .await?
            .into_iter()
            .map(|deployment| (deployment.subgraph_name.clone(), deployment))
            .collect::<HashMap<_, _>>();
        tracing::info!(deployments =? deployments, "found subgraph deployments");

        let protocols = protocol_infos
            .into_iter()
            .filter_map(|(slug, info)| {
                if let Some(deployment) = deployments.get(&info.subgraph_name) {
                    Some((
                        slug,
                        Protocol {
                            info,
                            subgraph_schema: deployment.schema_name.clone(),
                        },
                    ))
                } else {
                    tracing::warn!(
                        "protocol '{}' with subgraph_name '{}' not found in subgraph deployments",
                        slug,
                        info.subgraph_name
                    );
                    None
                }
            })
            .collect::<HashMap<_, _>>();

        let networks = networks.into_iter()
            .filter_map(|(chain_id, network)| {
                let (found_protocols, unknown_protocols): (Vec<_>, _) = network
                    .use_protocols
                    .into_iter()
                    .partition(|protocol_name| protocols.contains_key(protocol_name));
                if !unknown_protocols.is_empty() {
                    tracing::warn!("found unknown protocols for network with id={chain_id}: {unknown_protocols:?}")
                }
                if let Some(use_protocols) = NonEmpty::collect(found_protocols) {
                    Some(
                        (chain_id, Network {
                            blockscout_client: network.blockscout_client,
                            use_protocols,
                        })
                    )
                } else {
                    tracing::warn!("skip network with id={chain_id} since no protocols found");
                    None
                }
            })
            .collect::<HashMap<_, _>>();

        tracing::info!(networks =? networks.keys().collect::<Vec<_>>(), "initialized subgraph reader");
        let protocoler = Protocoler::initialize(networks, protocols)?;
        let this = Self::new(pool, protocoler);
        this.init_cache().await.context("init cache tables")?;
        Ok(this)
    }

    pub fn new(pool: Arc<PgPool>, protocoler: Protocoler) -> Self {
        Self { pool, protocoler }
    }

    pub async fn refresh_cache(&self) -> Result<(), anyhow::Error> {
        for protocol in self.iter_protocols() {
            let schema = &protocol.subgraph_schema;
            let address_resolve_technique = &protocol.info.address_resolve_technique;
            tracing::info!(
                address_resolve_technique =? address_resolve_technique,
                "refreshing cache table for schema {schema}"
            );
            match address_resolve_technique {
                AddressResolveTechnique::ReverseRegistry => {
                    sql::AddrReverseNamesView::refresh_view(self.pool.as_ref(), schema)
                        .await
                        .context(format!(
                            "failed to update AddrReverseNamesView for schema {schema}"
                        ))?;
                }
                AddressResolveTechnique::AllDomains => {
                    sql::AddressNamesView::refresh_view(self.pool.as_ref(), schema)
                        .await
                        .context(format!(
                            "failed to update AddressNamesView for schema {schema}"
                        ))?;
                }
            }
        }
        Ok(())
    }

    #[instrument(skip_all, err, level = "info")]
    pub async fn init_cache(&self) -> Result<(), anyhow::Error> {
        for protocol in self.iter_protocols() {
            let schema = &protocol.subgraph_schema;
            let address_resolve_technique = &protocol.info.address_resolve_technique;
            tracing::info!("start initializing cache table for schema {schema}");
            match address_resolve_technique {
                AddressResolveTechnique::ReverseRegistry => {
                    sql::AddrReverseNamesView::create_view(self.pool.as_ref(), schema)
                        .await
                        .context(format!(
                            "failed to create AddrReverseNamesView for schema {schema}"
                        ))?;
                }
                AddressResolveTechnique::AllDomains => {
                    sql::AddressNamesView::create_view(self.pool.as_ref(), schema)
                        .await
                        .context(format!(
                            "failed to create AddressNamesView for schema {schema}"
                        ))?;
                }
            }
        }
        Ok(())
    }

    pub fn iter_protocols(&self) -> impl Iterator<Item = &Protocol> {
        self.protocoler.iter_protocols()
    }

    pub fn protocols_of_network(
        &self,
        network_id: i64,
    ) -> Result<NonEmpty<DeployedProtocol>, ProtocolError> {
        self.protocoler.protocols_of_network(network_id, None)
    }
}

impl SubgraphReader {
    pub async fn get_domain(
        &self,
        input: GetDomainInput,
    ) -> Result<Option<GetDomainOutput>, SubgraphReadError> {
        let name = self.protocoler.main_name_in_network(
            &input.name,
            input.network_id,
            input.protocol_id.clone().map(|p| nonempty![p]),
        )?;
        let maybe_domain: Option<DetailedDomain> =
            sql::get_domain(self.pool.as_ref(), &name, &input)
                .await?
                .map(|domain| patch_detailed_domain(self.pool.clone(), domain, &name));
        if let Some(domain) = maybe_domain {
            let tokens = extract_tokens_from_domain(
                &domain,
                name.deployed_protocol.protocol.info.native_token_contract,
            )
            .map_err(|e| anyhow!("failed to extract domain tokens: {e}"))?;
            Ok(Some(GetDomainOutput {
                tokens,
                domain,
                protocol: name.deployed_protocol.protocol.clone(),
                deployment_network: name.deployed_protocol.deployment_network.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_domain_history(
        &self,
        input: GetDomainHistoryInput,
    ) -> Result<Vec<DomainEvent>, SubgraphReadError> {
        let name = self.protocoler.main_name_in_network(
            &input.name,
            input.network_id,
            input.protocol_id.clone().map(|p| nonempty![p]),
        )?;
        let domain_txns: Vec<DomainEventTransaction> = sql::find_transaction_events(
            self.pool.as_ref(),
            name.deployed_protocol.protocol,
            &name.inner,
            &input,
        )
        .await?;
        let domain_events = events_from_transactions(
            name.deployed_protocol
                .deployment_network
                .blockscout_client
                .clone(),
            domain_txns,
        )
        .await?;
        Ok(domain_events)
    }

    pub async fn lookup_domain_name(
        &self,
        input: LookupDomainInput,
    ) -> Result<PaginatedList<LookupOutput>, SubgraphReadError> {
        let find_domains_input = if let Some(name) = input.name {
            match self.protocoler.names_options_in_network(
                &name,
                input.network_id,
                input.maybe_filter_protocols,
            ) {
                Ok(name_options) => sql::FindDomainsInput::Names(name_options),
                Err(_) => return Ok(PaginatedList::empty()),
            }
        } else {
            let protocols = self
                .protocoler
                .protocols_of_network(input.network_id, input.maybe_filter_protocols)?;
            sql::FindDomainsInput::Protocols(protocols.map(|p| p.protocol).into_iter().collect())
        };

        let domains = sql::find_domains(
            self.pool.as_ref(),
            find_domains_input.clone(),
            input.only_active,
            Some(&input.pagination),
        )
        .await?
        .into_iter()
        .map(|domain| {
            // if domain is found by name, patch it with user input
            if let sql::FindDomainsInput::Names(names) = &find_domains_input {
                if let Some(from_user) = names.iter().find(|name| {
                    name.inner.id == domain.id
                        && name.deployed_protocol.protocol.info.slug == domain.protocol_slug
                }) {
                    return patch_domain(self.pool.clone(), domain, from_user);
                }
            };
            domain
        });
        let output = lookup_output_from_domains(domains, &self.protocoler)?;
        let paginated = input
            .pagination
            .paginate_result(output)
            .context("paginating result")?;
        Ok(paginated)
    }

    pub async fn lookup_address(
        &self,
        input: LookupAddressInput,
    ) -> Result<PaginatedList<LookupOutput>, SubgraphReadError> {
        if address_should_be_ignored(&input.address) {
            return Ok(PaginatedList::empty());
        }
        let protocols = self
            .protocoler
            .protocols_of_network(input.network_id, input.maybe_filter_protocols.clone())?
            .map(|p| p.protocol);
        let domains = sql::find_resolved_addresses(self.pool.as_ref(), protocols, &input).await?;
        let output = lookup_output_from_domains(domains, &self.protocoler)?;
        let paginated = input
            .pagination
            .paginate_result(output)
            .context("paginating result")?;
        Ok(paginated)
    }

    pub async fn get_address(
        &self,
        input: GetAddressInput,
    ) -> Result<Option<GetDomainOutput>, SubgraphReadError> {
        let protocols = self
            .protocoler
            .protocols_of_network(
                input.network_id,
                input.protocol_id.clone().map(|p| nonempty![p]),
            )?
            .map(|p| p.protocol);
        let maybe_domain_name =
            resolve_addresses(self.pool.as_ref(), protocols, vec![input.address])
                .await?
                .into_iter()
                .next()
                .map(|d| d.domain_name);
        if let Some(domain_name) = maybe_domain_name {
            let result = self
                .get_domain(GetDomainInput {
                    network_id: input.network_id,
                    name: domain_name,
                    only_active: true,
                    // protocol will be resolved automatically
                    protocol_id: None,
                })
                .await?
                .ok_or_else(|| {
                    anyhow!(
                        "batch search found domain for address, but detailed domain info not found"
                    )
                })?;
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    pub async fn count_domains_by_address(
        &self,
        network_id: i64,
        address: Address,
        resolved_to: bool,
        owned_by: bool,
    ) -> Result<i64, SubgraphReadError> {
        let protocols = self
            .protocoler
            .protocols_of_network(network_id, None)?
            .map(|p| p.protocol);
        let only_active = true;
        let count = sql::count_domains_by_address(
            self.pool.as_ref(),
            protocols,
            address,
            only_active,
            resolved_to,
            owned_by,
        )
        .await?;
        Ok(count)
    }

    pub async fn batch_resolve_address_names(
        &self,
        input: BatchResolveAddressNamesInput,
    ) -> Result<BTreeMap<String, String>, SubgraphReadError> {
        let protocols = self
            .protocoler
            .protocols_of_network(input.network_id, None)?
            .map(|p| p.protocol);
        // remove duplicates
        let addresses = remove_addresses_from_batch(input.addresses);
        let addresses_len = addresses.len();
        let result = resolve_addresses(self.pool.as_ref(), protocols, addresses).await?;

        let address_to_name: BTreeMap<String, String> = iter_to_map(
            result
                .into_iter()
                .map(|d| (d.resolved_address, d.domain_name)),
        );
        tracing::debug!(address_to_name =? address_to_name, "{}/{addresses_len} names found from batch request", address_to_name.len());
        Ok(address_to_name)
    }
}

// remove duplicates, remove unresolvable addresses, take only MAX_RESOLVE_ADDRESSES
fn remove_addresses_from_batch(addresses: impl IntoIterator<Item = Address>) -> Vec<Address> {
    addresses
        .into_iter()
        .filter(|addr| !address_should_be_ignored(addr))
        .collect::<HashSet<Address>>()
        .into_iter()
        .take(MAX_RESOLVE_ADDRESSES)
        .collect()
}

fn address_should_be_ignored(address: &Address) -> bool {
    let str = format!("{address:#x}");
    UNRESOLVABLE_ADDRESSES_PREFIXES
        .iter()
        .any(|prefix| str.starts_with(prefix))
}

// converting vector to hashmap with deduplication logic of first come first serve
fn iter_to_map<K, V>(iter: impl IntoIterator<Item = (K, V)>) -> BTreeMap<K, V>
where
    K: Eq + std::hash::Hash + std::cmp::Ord,
{
    let mut map = BTreeMap::new();
    for (key, value) in iter {
        map.entry(key).or_insert(value);
    }
    map
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
        .map_err(|e| anyhow!(e))?
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

fn lookup_output_from_domains(
    domains: impl IntoIterator<Item = Domain>,
    protocoler: &Protocoler,
) -> Result<Vec<LookupOutput>, anyhow::Error> {
    domains
        .into_iter()
        .map(|domain| {
            let protocol = protocoler
                .protocol_by_slug(&domain.protocol_slug)
                .ok_or_else(|| anyhow!("protocol not found"))?;
            Ok(LookupOutput {
                domain,
                protocol: protocol.protocol.clone(),
                deployment_network: protocol.deployment_network.clone(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        protocols::DomainNameOnProtocol, subgraphs_reader::sql, test_utils::mocked_reader,
    };
    use ethers::types::Address;
    use pretty_assertions::assert_eq;

    const DEFAULT_CHAIN_ID: i64 = 1;

    #[sqlx::test(migrations = "tests/migrations")]
    async fn get_domain_works(pool: PgPool) {
        let reader = mocked_reader(pool).await;

        // get vitalik domain
        let name = "vitalik.eth".to_string();
        let result = reader
            .get_domain(GetDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name,
                only_active: false,
                protocol_id: None,
            })
            .await
            .expect("failed to get vitalik domain")
            .expect("domain not found");
        let domain = result.domain;
        assert_eq!(domain.name.as_deref(), Some("vitalik.eth"));
        assert_eq!(
            domain.resolved_address.as_deref(),
            Some("0xd8da6bf26964af9d7eed9e03e53415d37aa96045")
        );
        let other_addresses: HashMap<String, String> = serde_json::from_value(serde_json::json!({
            "ETH": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045",
            "RSK": "0xf0d485009714cE586358E3761754929904D76B9D",
        }))
        .unwrap();
        assert_eq!(domain.other_addresses, other_addresses.into());

        // get expired domain
        let name = "expired.eth".to_string();
        let result = reader
            .get_domain(GetDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name: name.clone(),
                only_active: false,
                protocol_id: None,
            })
            .await
            .expect("failed to get expired domain")
            .expect("expired domain not found");
        let domain = result.domain;
        assert!(
            domain.is_expired,
            "expired domain has is_expired=false: {:?}",
            domain
        );
        // since no info in multicoin_addr_changed
        assert!(domain.other_addresses.is_empty());

        // get expired domain with only_active filter
        let result = reader
            .get_domain(GetDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name,
                only_active: true,
                protocol_id: None,
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
        let reader = mocked_reader(pool).await;

        let result = reader
            .lookup_domain_name(LookupDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name: Some("vitalik.eth".to_string()),
                only_active: false,
                pagination: Default::default(),
                maybe_filter_protocols: None,
            })
            .await
            .expect("failed to get vitalik domains");
        assert_eq!(result.next_page_token, None);
        let result = result.items;
        assert_eq!(
            vec![Some("vitalik.eth")],
            result
                .iter()
                .map(|output| output.domain.name.as_deref())
                .collect::<Vec<_>>(),
        );
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn lookup_addresses_works(pool: PgPool) {
        let reader = mocked_reader(pool).await;

        let result = reader
            .lookup_address(LookupAddressInput {
                network_id: DEFAULT_CHAIN_ID,
                address: addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"),
                resolved_to: true,
                owned_by: false,
                only_active: false,
                pagination: Default::default(),
                maybe_filter_protocols: None,
            })
            .await
            .expect("failed to get vitalik domains");
        assert_eq!(result.next_page_token, None);
        let result = result.items;
        assert_eq!(
            result
                .iter()
                .map(|output| output.domain.name.as_deref())
                .collect::<Vec<_>>(),
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
                maybe_filter_protocols: None,
            })
            .await
            .expect("failed to get vitalik domains");
        assert_eq!(result.next_page_token, None);
        let result = result.items;
        assert_eq!(
            result
                .iter()
                .map(|output| output.domain.name.as_deref())
                .collect::<Vec<_>>(),
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
                maybe_filter_protocols: None,
            })
            .await
            .expect("failed to get expired domains");
        assert_eq!(result.next_page_token, None);
        let result = result.items;
        // expired domain shoudn't be returned as resolved
        assert_eq!(
            result
                .iter()
                .map(|output| output.domain.name.as_deref())
                .collect::<Vec<_>>(),
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
                maybe_filter_protocols: None,
            })
            .await
            .expect("failed to get expired domains");
        assert_eq!(result.next_page_token, None);
        let result = result.items;
        // expired domain shoudn't be returned as resolved
        assert_eq!(
            result
                .iter()
                .map(|output| output.domain.name.as_deref())
                .collect::<Vec<_>>(),
            vec![]
        );
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn get_domain_history_works(pool: PgPool) {
        let reader = mocked_reader(pool).await;
        let name = "vitalik.eth".to_string();
        let history = reader
            .get_domain_history(GetDomainHistoryInput {
                network_id: DEFAULT_CHAIN_ID,
                name,
                sort: Default::default(),
                order: Default::default(),
                protocol_id: None,
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
        let reader = mocked_reader(pool).await;

        let addresses = [
            // `test.eth` has resolved_address of this address
            // however `{addr}.addr.reverse` contains `this-is-not-test.eth` in reverse record
            // so it should not be resolved as any name
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
        let unresolved_label = "you-dont-know-this-label";
        let unresolved = "you-dont-know-this-label.eth";
        let reader = mocked_reader(pool).await;
        let protocol = reader
            .protocoler
            .protocols_of_network(DEFAULT_CHAIN_ID, None)
            .expect("failed to get protocol")
            .head;

        // Make sure that database contains unresolved domain
        let domain = sql::get_domain(
            reader.pool.as_ref(),
            &DomainNameOnProtocol::new(unresolved, protocol).expect("unresolved name is valid"),
            &GetDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name: unresolved.to_string(),
                only_active: false,
                protocol_id: None,
            },
        )
        .await
        .expect("failed to get domain")
        .expect("unresolved domain not found using sql");
        assert_eq!(
            domain.name.as_deref(),
            Some("[0b0e081f36b3970ff8e337f0ff7bdfad321a702fa00916b6ccfc47877144f7ad].eth")
        );
        assert_eq!(domain.label_name, None,);

        // After reader requests domain should be resolved
        let result = reader
            .get_domain(GetDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name: unresolved.to_string(),
                only_active: false,
                protocol_id: None,
            })
            .await
            .expect("failed to get domain")
            .expect("unresolved domain not found using reader");
        let domain = result.domain;
        assert_eq!(domain.name.as_deref(), Some(unresolved));
        assert_eq!(domain.label_name.as_deref(), Some(unresolved_label));

        // Make sure that unresolved name in database became resolved
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let domain = sql::get_domain(
            reader.pool.as_ref(),
            &DomainNameOnProtocol::new(unresolved, protocol).expect("unresolved name is valid"),
            &GetDomainInput {
                network_id: DEFAULT_CHAIN_ID,
                name: unresolved.to_string(),
                only_active: false,
                protocol_id: None,
            },
        )
        .await
        .expect("failed to get domain")
        .expect("unresolved domain not found using sql");
        assert_eq!(domain.name.as_deref(), Some(unresolved));
        assert_eq!(domain.label_name.as_deref(), Some(unresolved_label));
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
