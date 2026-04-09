use crate::{
    protocols::{
        AddressResolveTechnique, DeployedProtocol, Network, Protocol, ProtocolError, ProtocolInfo,
        Protocoler,
    },
    subgraph::{
        sql::{self, AdditionalTable, CachedView},
        SubgraphPatcher,
    },
};
use anyhow::Context;
use nonempty::NonEmpty;
use sqlx::postgres::PgPool;
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;

pub struct SubgraphReader {
    pub(super) pool: Arc<PgPool>,
    pub(super) protocoler: Protocoler,
    pub(super) patcher: SubgraphPatcher,
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
            .map(|(network_id, network)| {
                let (found_protocols, unknown_protocols): (Vec<_>, _) = network
                    .use_protocols
                    .into_iter()
                    .partition(|protocol_name| protocols.contains_key(protocol_name));
                if !unknown_protocols.is_empty() {
                    tracing::warn!("found unknown or disabled protocols for network with id={network_id}: {unknown_protocols:?}")
                }
                (network_id, Network {
                    network_id,
                    blockscout_client: network.blockscout_client,
                    use_protocols: found_protocols,
                    rpc_url: network.rpc_url,
                })
            })
            .collect::<HashMap<_, _>>();

        tracing::info!(networks =? networks.keys().collect::<Vec<_>>(), "initialized subgraph reader");
        let protocoler = Protocoler::initialize(networks, protocols)?;
        let patcher = SubgraphPatcher::new();
        let this = Self::new(pool, protocoler, patcher);
        this.init_cache().await.context("init cache tables")?;
        Ok(this)
    }

    pub fn new(pool: Arc<PgPool>, protocoler: Protocoler, patcher: SubgraphPatcher) -> Self {
        Self {
            pool,
            protocoler,
            patcher,
        }
    }

    /// PostgreSQL pool handle (for diagnostics from `bens-server`).
    pub fn pg_pool(&self) -> &PgPool {
        self.pool.as_ref()
    }

    #[instrument(skip(self), err, level = "info")]
    pub async fn refresh_cache(&self) -> Result<(), anyhow::Error> {
        let job_started = std::time::Instant::now();
        let mut protocols_refreshed: u32 = 0;
        for protocol in self.iter_protocols() {
            let schema = &protocol.subgraph_schema;
            let slug = protocol.info.slug.as_str();
            let address_resolve_technique = &protocol.info.address_resolve_technique;
            tracing::info!(
                target: "bens.refresh_cache",
                protocol_slug = slug,
                subgraph_schema = schema,
                ?address_resolve_technique,
                "refresh_cache: starting protocol"
            );
            let step_started = std::time::Instant::now();
            match address_resolve_technique {
                AddressResolveTechnique::ReverseRegistry => {
                    sql::AddrReverseNamesView::refresh_view(self.pool.as_ref(), schema)
                        .await
                        .context(format!(
                            "failed to update AddrReverseNamesView for schema {schema}"
                        ))?;
                    protocols_refreshed += 1;
                }
                AddressResolveTechnique::AllDomains => {
                    sql::AddressNamesView::refresh_view(self.pool.as_ref(), schema)
                        .await
                        .context(format!(
                            "failed to update AddressNamesView for schema {schema}"
                        ))?;
                    protocols_refreshed += 1;
                }
                AddressResolveTechnique::Addr2Name => {
                    tracing::debug!(
                        target: "bens.refresh_cache",
                        protocol_slug = slug,
                        subgraph_schema = schema,
                        "refresh_cache: skipped (addr2name has no materialized view)"
                    );
                }
                AddressResolveTechnique::PrimaryNameRecord => {
                    tracing::debug!(
                        target: "bens.refresh_cache",
                        protocol_slug = slug,
                        subgraph_schema = schema,
                        "refresh_cache: skipped (primary_name_record)"
                    );
                }
            }
            tracing::info!(
                target: "bens.refresh_cache",
                protocol_slug = slug,
                subgraph_schema = schema,
                elapsed_ms = step_started.elapsed().as_millis() as u64,
                "refresh_cache: finished protocol step"
            );
        }
        tracing::info!(
            target: "bens.refresh_cache",
            protocols_refreshed,
            elapsed_ms = job_started.elapsed().as_millis() as u64,
            "refresh_cache: job completed"
        );
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
                AddressResolveTechnique::Addr2Name => {
                    sql::Addr2NameTable::create_table(self.pool.as_ref(), schema)
                        .await
                        .context(format!(
                            "failed to create Addr2NameTable for schema {schema}"
                        ))?;
                }
                AddressResolveTechnique::PrimaryNameRecord => {
                    // primary_name_record table is created by the subgraph itself
                    continue;
                }
            }
        }
        Ok(())
    }

    pub fn iter_protocols(&self) -> impl Iterator<Item = &Protocol> {
        self.protocoler.iter_protocols()
    }

    pub fn iter_deployed_protocols(&self) -> impl Iterator<Item = DeployedProtocol<'_>> {
        self.protocoler.iter_deployed_protocols()
    }

    pub fn protocols_of_network(
        &'_ self,
        network_id: i64,
    ) -> Result<NonEmpty<DeployedProtocol<'_>>, ProtocolError> {
        self.protocoler.protocols_of_network(network_id, None)
    }
}
