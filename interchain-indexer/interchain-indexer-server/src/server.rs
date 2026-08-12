// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{
    create_provider_pools_from_chains,
    indexers::{IndexingTarget, enumerate_indexing_targets, reconcile_catchup_floors},
    load_bridges_from_file, load_chains_from_file,
    proto::{
        health_actix::route_health, health_server::HealthServer,
        interchain_service_actix::route_interchain_service,
        interchain_service_server::InterchainServiceServer,
        interchain_statistics_service_server::InterchainStatisticsServiceServer,
        status_service_server::StatusServiceServer,
    },
    services::{
        HealthService, InterchainServiceImpl, InterchainStatisticsServiceImpl, StatusServiceImpl,
        collect_indexing_progress,
    },
    settings::Settings,
    spawn_configured_indexers,
};
use blockscout_endpoint_swagger::route_swagger;
use blockscout_service_launcher::{
    database, launcher, launcher::LaunchSettings, tracing as bs_tracing,
};
use chrono::NaiveDateTime;
use interchain_indexer_entity::{bridge_contracts, bridges, chains};
use interchain_indexer_logic::{
    ChainInfoService, IndexedChains, InterchainDatabase, StatsReadSettings, StatsService,
    TokenInfoService,
    indexer::metrics::{
        FAILED_BLOCKS, INDEXER_CATCHUP_BLOCKS_REMAINING, INDEXER_CATCHUP_PROGRESS,
        OLDEST_OPEN_HOLE_AGE_SECONDS,
    },
};
use interchain_indexer_proto::blockscout::interchain_indexer::v1::{
    interchain_statistics_service_actix::route_interchain_statistics_service,
    status_service_actix::route_status_service,
};
use migration::Migrator;
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};
const SERVICE_NAME: &str = "interchain_indexer";

/// Refresh interval for the indexing-progress gauges. A module constant, not a
/// setting: there is no operational decision behind it and a setting would
/// grow the ENV surface.
const PROGRESS_METRICS_REFRESH: Duration = Duration::from_secs(60);

/// Spawns a Tokio task that recomputes `stats_chains` on a fixed interval.
///
/// The **first** recomputation runs immediately after startup wiring (before the first sleep),
/// so fresh stats are available without waiting a full period. Subsequent runs wait
/// `period_secs` after each attempt (success or failure). If `period_secs` is `0`, does nothing.
fn spawn_stats_chains_recalculation_worker(stats: Arc<StatsService>, period_secs: u64) {
    if period_secs == 0 {
        tracing::info!("stats_chains_recalculation_period_secs is 0: periodic refresh disabled");
        return;
    }

    tokio::spawn(async move {
        loop {
            tracing::info!("stats_chains recomputation started");
            match stats.recompute_stats_chains().await {
                Ok(()) => tracing::info!("stats_chains recomputation succeeded"),
                Err(err) => tracing::error!(
                    err = ?err,
                    "stats_chains recomputation failed; keeping previous rows, retrying after interval"
                ),
            }
            tokio::time::sleep(Duration::from_secs(period_secs)).await;
        }
    });
}

/// Refreshes `interchain_indexer_catchup_progress` /
/// `interchain_indexer_catchup_blocks_remaining` on a fixed interval, from
/// the same `collect_indexing_progress` join the RPC handler uses so the two
/// can never disagree. Also refreshes `interchain_indexer_failed_blocks` /
/// `interchain_indexer_oldest_open_hole_age_seconds`.
///
/// The two failure-ledger gauges used to be refreshed by `RangeDriver`'s
/// retry tick, which is reachable only from a live driver loop — so a pair
/// whose indexer never started emitted no `failed_blocks` series at all, and
/// a driver that `bail!`ed out of its loop froze both gauges forever,
/// including `oldest_open_hole_age_seconds`, the sole detector of a retry
/// pass that has stopped converging. Refreshing them here instead, keyed off
/// `targets` (every configured `(bridge, chain)` pair, config-driven, the
/// same enumeration `collect_indexing_progress` uses), fixes both: this
/// worker runs regardless of any driver's liveness, so every configured
/// target gets both series.
///
/// Gauges are written only here. A gauge refreshed from a request handler
/// would be frozen between calls, which is worse than not having it.
fn spawn_indexing_progress_metrics_worker(
    db: Arc<InterchainDatabase>,
    targets: Arc<Vec<IndexingTarget>>,
) {
    tokio::spawn(async move {
        loop {
            match collect_indexing_progress(&db, &targets, None, None).await {
                Ok(items) => {
                    for item in items {
                        let bridge_label = item.bridge_id.to_string();
                        let chain_label = item.chain_id.to_string();
                        INDEXER_CATCHUP_PROGRESS
                            .with_label_values(&[&bridge_label, &chain_label])
                            .set(item.catchup_progress_percent);
                        INDEXER_CATCHUP_BLOCKS_REMAINING
                            .with_label_values(&[&bridge_label, &chain_label])
                            .set(item.catchup_blocks_remaining as f64);
                    }
                }
                Err(err) => tracing::error!(
                    err = ?err,
                    "indexing-progress metrics refresh failed; keeping previous gauge values, retrying after interval"
                ),
            }
            refresh_failure_ledger_gauges(&db, &targets).await;
            tokio::time::sleep(PROGRESS_METRICS_REFRESH).await;
        }
    });
}

/// Refresh the two failure-ledger gauges from `indexer_failure_totals`,
/// covering every configured target rather than only the pairs a live
/// `RangeDriver` happens to be looping over.
///
/// **Fetch first, decide, then apply** — never zero the gauges before the
/// query is known to have succeeded. A pool blip on the totals query must
/// leave the previous gauge values in place; zeroing first would make a
/// transient DB error look like "every hole just vanished," clearing exactly
/// the alert this gauge exists to raise. The decision (including which
/// absent pairs become explicit zeroes) is the pure, unit-tested
/// `gauge_refresh_values`; this function only performs the `.await` and the
/// trivial `.set()` calls around it.
async fn refresh_failure_ledger_gauges(db: &InterchainDatabase, targets: &[IndexingTarget]) {
    let totals = db.indexer_failure_totals(None, None).await;
    let now = chrono::Utc::now().naive_utc();
    let pairs: Vec<(i32, i64)> = targets
        .iter()
        .map(|target| (target.bridge_id, target.chain_id))
        .collect();

    if let Some(values) = gauge_refresh_values(&pairs, &totals, now) {
        for (bridge_id, chain_id, blocks, age_seconds) in values {
            let bridge_label = bridge_id.to_string();
            let chain_label = chain_id.to_string();
            FAILED_BLOCKS
                .with_label_values(&[&bridge_label, &chain_label])
                .set(blocks);
            OLDEST_OPEN_HOLE_AGE_SECONDS
                .with_label_values(&[&bridge_label, &chain_label])
                .set(age_seconds);
        }
    }

    // Matched on the result itself rather than on `gauge_refresh_values`
    // returning `None`: the two are equivalent only as long as `Err` stays
    // that function's sole `None` path, and an `unwrap_err` resting on that
    // coupling would panic this worker the moment a second `None` arm is
    // added (`.memory-bank/rules/error-handling.md`).
    if let Err(err) = totals {
        tracing::error!(
            err = ?err,
            "failed to refresh failure-ledger gauges; leaving previous values in place"
        );
    }
}

/// One gauge pair's value: `(bridge_id, chain_id, failed_blocks,
/// oldest_open_hole_age_seconds)`.
type GaugeValue = (i32, i64, f64, f64);

/// One `indexer_failure_totals` row: `(bridge_id, chain_id, blocks, oldest created_at)`.
type FailureTotalsRow = (i32, i64, u64, Option<NaiveDateTime>);

/// Outcome of an `indexer_failure_totals` query, as passed to
/// [`gauge_refresh_values`].
type FailureTotalsResult = anyhow::Result<Vec<FailureTotalsRow>>;

/// Decides the per-pair gauge values to apply for one totals-query outcome.
///
/// `Ok(totals)` -> `Some(values)`, one entry per `pairs`: a pair present in
/// `totals` gets its aggregate; a pair absent gets an explicit zero — a pair
/// whose last hole was just resolved does not appear in
/// `indexer_failure_totals`, so without this its `GaugeVec` child would stay
/// frozen at the last non-zero value. This also covers a pair whose indexer
/// never started: it is still in `pairs` (config-driven), so it gets an
/// explicit zero instead of no series at all.
///
/// `Err(_)` -> `None`, meaning "apply nothing, leave the gauges at their
/// previous values." Communicating this as `None` rather than falling
/// through to a zeroed `Vec` is what makes the "leave values untouched on
/// error" behaviour (defect: a failed query used to zero every gauge)
/// unit-testable without touching the process-wide `GaugeVec`s themselves
/// (`.memory-bank/rules/testing.md`).
fn gauge_refresh_values(
    pairs: &[(i32, i64)],
    totals: &FailureTotalsResult,
    now: NaiveDateTime,
) -> Option<Vec<GaugeValue>> {
    let totals = totals.as_ref().ok()?;

    let by_pair: HashMap<(i32, i64), (u64, Option<NaiveDateTime>)> = totals
        .iter()
        .map(|(bridge_id, chain_id, blocks, oldest)| ((*bridge_id, *chain_id), (*blocks, *oldest)))
        .collect();

    Some(
        pairs
            .iter()
            .map(|&(bridge_id, chain_id)| {
                let (blocks, age_seconds) = match by_pair.get(&(bridge_id, chain_id)) {
                    Some((blocks, oldest)) => {
                        let age = oldest
                            .map(|ts| (now - ts).num_seconds().max(0))
                            .unwrap_or(0);
                        (*blocks as f64, age as f64)
                    }
                    None => (0.0, 0.0),
                };
                (bridge_id, chain_id, blocks, age_seconds)
            })
            .collect(),
    )
}

#[derive(Clone)]
struct Router {
    health: Arc<HealthService>,
    interchain_service: Arc<InterchainServiceImpl>,
    stats_service: Arc<InterchainStatisticsServiceImpl>,
    status_service: Arc<StatusServiceImpl>,
    swagger_path: PathBuf,
}

impl Router {
    pub fn grpc_router(&self) -> tonic::transport::server::Router {
        tonic::transport::Server::builder()
            .add_service(HealthServer::from_arc(self.health.clone()))
            .add_service(InterchainServiceServer::from_arc(
                self.interchain_service.clone(),
            ))
            .add_service(InterchainStatisticsServiceServer::from_arc(
                self.stats_service.clone(),
            ))
            .add_service(StatusServiceServer::from_arc(self.status_service.clone()))
    }
}

impl launcher::HttpRouter for Router {
    fn register_routes(&self, service_config: &mut actix_web::web::ServiceConfig) {
        service_config.configure(|config| route_health(config, self.health.clone()));
        service_config
            .configure(|config| route_interchain_service(config, self.interchain_service.clone()));
        service_config.configure(|config| {
            route_interchain_statistics_service(config, self.stats_service.clone())
        });
        service_config
            .configure(|config| route_status_service(config, self.status_service.clone()));
        service_config.configure(|config| {
            route_swagger(
                config,
                self.swagger_path.clone(),
                "/api/v1/docs/swagger.yaml",
            )
        });
    }
}

pub async fn run(settings: Settings) -> Result<(), anyhow::Error> {
    bs_tracing::init_logs(SERVICE_NAME, &settings.tracing, &settings.jaeger)?;

    let health = Arc::new(HealthService::default());

    let db_connection =
        Arc::new(database::initialize_postgres::<Migrator>(&settings.database).await?);
    let interchain_db = InterchainDatabase::new(db_connection);
    let db = Arc::new(interchain_db.clone());

    let chains = load_chains_from_file(&settings.chains_config)?;
    // Single provider pool per chain, shared by the token info service and the
    // indexers. `DynProvider` is `Arc`-backed, so cloning the map shares the
    // same underlying failover state (one health task, one primary per chain).
    let chain_providers = create_provider_pools_from_chains(chains.clone()).await?;
    let token_info_service = Arc::new(TokenInfoService::new(
        db.clone(),
        chain_providers.clone(),
        settings.token_info.clone(),
    ));
    // `load_bridges_from_file` is pure file IO and is hoisted above stats
    // wiring so the indexed-chain set below can be built from it. Nothing else
    // moves: the startup backfill a few lines down still runs before
    // `upsert_chains` / `upsert_bridges` / `upsert_bridge_contracts`, because a
    // DB-derived indexed-chain set would be stale exactly when backfill needs
    // it (the DB has no bridge_contracts rows yet on a fresh deployment).
    let bridges = load_bridges_from_file(&settings.bridges_config)?;

    // Stats eligibility ("can missing evidence still arrive?") is derived from
    // this in-memory set, never from `bridge_contracts`. Include *all*
    // declared bridges, even disabled ones: `enabled` is an operational
    // switch, not a statement about observability, and a contract-less bridge
    // must end up present in the map with an empty set (restrictive), not
    // omitted (permissive) — see `IndexedChains::may_observe`.
    let indexed_chains = IndexedChains::from_bridges(bridges.iter().map(|b| {
        (
            b.bridge_id,
            b.contracts.iter().map(|c| c.chain_id).collect(),
        )
    }));
    // Iterate the map (not the config) so a `chain_count = 0` line proves the
    // bridge is *present* in the map, not merely declared in the config file.
    for (bridge_id, chain_count) in indexed_chains.bridge_chain_counts() {
        tracing::info!(bridge_id, chain_count, "stats indexed-chain set");
        if chain_count == 0 {
            tracing::warn!(
                bridge_id,
                "bridge declared with no configured contracts; every chain counts as unindexed for it and its partial data will be committed to stats"
            );
        }
    }
    indexed_chains.record_metrics();
    // Misconfiguration guard, not a safety net: with bridges configured but
    // zero total pairs, every bridge in the map has an empty set, so every
    // chain is unindexed and every pending transfer becomes countable.
    anyhow::ensure!(
        bridges.is_empty() || indexed_chains.pair_count() > 0,
        "stats indexed-chain set is empty while {} bridge(s) are configured in {}: every \
         chain would be treated as unindexed for every bridge and every pending transfer \
         would become countable",
        bridges.len(),
        settings.bridges_config.display()
    );

    // Shared by both API services below so the SQL predicate and the response
    // flag are computed from the exact same value.
    let indexed_chains = Arc::new(indexed_chains);

    let stats = Arc::new(StatsService::new(
        db.clone(),
        Some(token_info_service.clone()),
        StatsReadSettings {
            include_zero_chains: settings.stats.include_zero_chains,
        },
        (*indexed_chains).clone(),
    ));

    if settings.stats.backfill_on_start {
        tracing::info!(
            "stats.backfill_on_start enabled: running statistics projection; async token enrichment will run after each batch outside DB transactions"
        );
        stats
            .backfill_stats_until_idle_with_token_enrichment()
            .await?;
    }

    // Populate database with the chains, bridges and bridge contracts
    db.upsert_chains(
        chains
            .clone()
            .into_iter()
            .map(chains::ActiveModel::from)
            .collect::<Vec<chains::ActiveModel>>(),
    )
    .await?;
    db.upsert_bridges(
        bridges
            .clone()
            .iter()
            .map(|b| bridges::ActiveModel::from(b.clone()))
            .collect::<Vec<bridges::ActiveModel>>(),
    )
    .await?;

    // Enumerated once from the in-memory bridges config, before `bridges` is
    // moved into `InterchainServiceImpl::new` below. Shared by the
    // indexing-progress RPC handler and its periodic metrics worker so the
    // two can never disagree.
    let targets = Arc::new(enumerate_indexing_targets(&bridges));

    // Order-independent by construction: the reconciliation compares the
    // configured scan floor against `indexer_checkpoints.catchup_min_cursor`
    // and never reads `bridge_contracts`, so it does not care whether the
    // upsert below has run. An earlier version derived the previous floor from
    // `bridge_contracts` and had to run first — see `reconcile_catchup_floors`
    // for why that proxy was removed.
    reconcile_catchup_floors(&db, &bridges).await;

    let bridge_contracts: Vec<bridge_contracts::ActiveModel> = bridges
        .iter()
        .flat_map(|bridge| {
            bridge
                .contracts
                .iter()
                .map(move |contract| contract.to_active_model(bridge.bridge_id))
        })
        .collect();
    if !bridge_contracts.is_empty() {
        db.upsert_bridge_contracts(bridge_contracts.clone()).await?;
    }

    tracing::info!(
        "Loaded {} chains ({}), {} bridges ({}) and {} bridge contracts from \
         JSON files",
        chains.len(),
        chains
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<String>>()
            .join(", "),
        bridges.len(),
        bridges
            .iter()
            .map(|b| b.name.clone())
            .collect::<Vec<String>>()
            .join(", "),
        bridge_contracts.len(),
    );

    let chain_info_service = Arc::new(ChainInfoService::new(
        db.clone(),
        settings.chain_info.clone(),
    ));

    let indexers = spawn_configured_indexers(
        stats.clone(),
        &bridges,
        &chains,
        &chain_providers,
        &settings,
    )
    .await?;

    // let example = ExampleIndexer::new(
    //     db.clone(),
    //     bridges[0].bridge_id,
    //     chains_providers,
    //     Default::default(),
    // )?;

    // example.start_indexing().await?;

    let api_settings = settings.api.clone();
    let interchain_service = Arc::new(InterchainServiceImpl::new(
        db.clone(),
        token_info_service.clone(),
        chain_info_service.clone(),
        bridges,
        api_settings.clone(),
        indexed_chains.clone(),
    ));
    let stats_service = Arc::new(InterchainStatisticsServiceImpl::new(
        stats.clone(),
        api_settings,
        chain_info_service.clone(),
        indexed_chains.clone(),
    ));
    let status_service = Arc::new(StatusServiceImpl::new(
        indexers.clone(),
        db.clone(),
        targets.clone(),
    ));
    let router = Router {
        health,
        interchain_service,
        stats_service,
        status_service,
        swagger_path: settings.swagger_path,
    };

    let grpc_router = router.grpc_router();
    let http_router = router;

    let stats_chains_period_secs = settings.stats.chains_recalculation_period_secs;
    spawn_stats_chains_recalculation_worker(stats.clone(), stats_chains_period_secs);
    spawn_indexing_progress_metrics_worker(db.clone(), targets.clone());

    let launch_settings = LaunchSettings {
        service_name: SERVICE_NAME.to_string(),
        server: settings.server,
        metrics: settings.metrics,
        graceful_shutdown: Default::default(),
    };

    let launch_result = launcher::launch(launch_settings, http_router, grpc_router).await;

    for indexer in indexers {
        indexer.stop().await;
    }

    //example.stop_indexing().await;

    launch_result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(secs_since_epoch: i64) -> NaiveDateTime {
        chrono::DateTime::from_timestamp(secs_since_epoch, 0)
            .unwrap()
            .naive_utc()
    }

    #[test]
    fn gauge_refresh_values_uses_the_aggregate_for_a_pair_present_in_totals() {
        let now = ts(10_000);
        let oldest = ts(9_000);
        let totals: FailureTotalsResult = Ok(vec![(1, 42, 250, Some(oldest))]);

        let values =
            gauge_refresh_values(&[(1, 42)], &totals, now).expect("Ok totals must yield Some");

        assert_eq!(values, vec![(1, 42, 250.0, 1_000.0)]);
    }

    #[test]
    fn gauge_refresh_values_zeroes_a_configured_pair_absent_from_totals() {
        let now = ts(10_000);
        // (1, 42) has an open hole; (1, 7) is configured — including a pair
        // whose indexer never started — but has never recorded a failure and
        // therefore does not appear in the aggregate at all.
        let totals: FailureTotalsResult = Ok(vec![(1, 42, 250, Some(ts(9_000)))]);

        let values = gauge_refresh_values(&[(1, 42), (1, 7)], &totals, now)
            .expect("Ok totals must yield Some");

        assert_eq!(values.len(), 2);
        assert!(
            values.contains(&(1, 7, 0.0, 0.0)),
            "a healthy configured pair must get an explicit zero, not be omitted: {values:?}"
        );
    }

    #[test]
    fn gauge_refresh_values_returns_none_on_error_so_callers_leave_values_untouched() {
        let now = ts(10_000);
        let totals: FailureTotalsResult = Err(anyhow::anyhow!("pool blip"));

        let values = gauge_refresh_values(&[(1, 42)], &totals, now);

        assert!(
            values.is_none(),
            "a failed totals query must not produce any values to apply: {values:?}"
        );
    }
}
