# Architecture

## High-Level Data Flow

```text
Indexer DB(s) (Blockscout / multichain-aggregator / interchain-indexer / +CCTX)
    ↓ (RemoteDatabaseSource: raw SQL Statement built per data source)
RemoteQueryBehaviour::query_data
    ↓ (data_manipulation: map / delta / sum / resolutions / last_point / ...)
DataSource::update_recursively (LocalDbChartSource)
    ↓ (persist points, batched)
Stats DB (entity::chart_data / entity::charts)
    ↓ (ReadService queries, request-time range/limit handling)
REST (actix-prost) / gRPC (tonic) API
```

Updates and reads for a group of related charts are driven together by an
`UpdateGroup` (`stats/src/update_group.rs`), constructed by
`stats-server/src/runtime_setup.rs` from `charts.json` + `update_groups.json`
and run on a cron schedule by `UpdateService` (`stats-server/src/update_service.rs`).

## Core Abstractions

### `DataSource` trait

Location: `stats/src/data_source/source.rs` (re-exported from
`stats/src/data_source/mod.rs`)

Every node in the chart dependency DAG (a chart, a chart's local
dependencies, or a "pull data from indexer DB" leaf) implements `DataSource`.
Nodes declare `MainDependencies`/`ResolutionDependencies` as associated types,
so the whole dependency graph is expressed as nested Rust generic types
rather than a runtime graph structure. `DataSource` provides
`init_recursively`, `update_recursively`, and `query_data` — each is expected
to recurse into dependencies first, then act on itself, so that data pulled
from a dependency is always relevant to the same update.

See the module doc comment in `stats/src/data_source/mod.rs` for the full
rationale (composability as a DAG, why Rust types double as DAG nodes, and
why the update timestamp is threaded through `UpdateContext`).

### `ChartProperties` trait

Location: `stats/src/charts/chart.rs`

Static per-chart metadata: `Resolution` (associated type, e.g. `NaiveDate`,
`Week`, `Month`, `Year` — see `types::Timespan`), `chart_type()`
(`ChartType::Line` or `ChartType::Counter`, from
`entity::sea_orm_active_enums`), `missing_date_policy()` (`FillZero` or
`FillPrevious`), `indexing_status_requirement()`, `key()` (unique
`ChartKey { name, resolution }`). A "chart" in this codebase is, informally,
anything implementing both `DataSource` and `ChartProperties` and stored via
`kinds::local_db`.

### `kinds` — the reusable `DataSource` building blocks

Location: `stats/src/data_source/kinds/`

- `remote_db/` — `RemoteDatabaseSource<Behaviour>` wraps a
  `RemoteQueryBehaviour`/`StatementFromRange`/`StatementForOne`/... impl that
  builds a raw SQL `Statement` against the indexer DB and parses the result.
  `db_choice.rs` (`UsePrimaryDB` / `UseZetachainCctxDB`) picks which indexer
  connection (`cx.indexer_db` vs `cx.second_indexer_db`) a statement runs
  against.
- `local_db/` — `LocalDbChartSource` and its aliases
  (`DirectVecLocalDbChartSource`, `DirectPointLocalDbChartSource`,
  `DirectPointLocalDbChartSourceWithEstimate`, `DailyCumulativeLocalDbChartSource`,
  `DirectPointCachedLocalDbChartSource`) persist a dependency's output into
  the stats DB (`entity::chart_data`), with pluggable `CreateBehaviour`/
  `UpdateBehaviour` and update batching (`parameters::update::batching`,
  e.g. `Batch30Days`, `Batch36Months`).
- `data_manipulation/` — pure transforms composed over other data sources:
  `map` (`MapParseTo`, `MapToString`, `StripExt`, `MapDivide`,
  `ClampNonNegative`, `UnwrapOr`), `delta::Delta`, `sum_point::Sum`,
  `last_point::LastPoint`, `filter_deducible::FilterDeducible`,
  `resolutions::{average, last_value, sum}` (roll a daily source up to
  week/month/year).
- `auxiliary/` — `cumulative.rs` and small helper sources.

### Update groups

Location: `stats/src/update_group.rs`, `stats/src/update_groups*.rs`

`construct_update_group!{ GroupName { charts: [A, B, ...] } }` generates a
zero-sized type implementing `UpdateGroup`: `create_charts`/`update_charts`
recurse into every listed member's `DataSource` impl for the members that are
enabled. Groups exist so that charts sharing an expensive dependency (e.g.
"all blocks in range") update together and reuse that dependency's single
query/write, and so update timestamps are consistent across related charts —
see the doc comments at the top of `stats/src/update_groups.rs` and in
`update_group.rs` for the worked example and the "always include
dependencies as members" recommendation.

`SyncUpdateGroup` wraps a group with per-chart-name `tokio::sync::Mutex`es
(one per `DataSource::mutex_id()` in the group + dependencies), always locked
in lexicographic name order (`BTreeMap` iteration) to make concurrent group
updates deadlock-free as long as every group goes through `SyncUpdateGroup`
and the mutex map is shared across groups (built once by
`stats-server/src/runtime_setup.rs`).

`stats/src/update_groups.rs` holds the Blockscout-instance groups (via
`singleton_groups!` for standalone charts and `construct_update_group!` for
multi-chart groups); `update_groups_interchain.rs` and
`update_groups_multichain.rs` hold the per-mode equivalents for
`Interchain`/`MultichainAggregator` mode charts.

### `UpdateContext` / `UpdateParameters`

Location: `stats/src/data_source/types.rs`

`UpdateParameters` is the caller-supplied bundle (stats DB, indexer DB(s),
`Mode`, `multichain_filter`, `interchain_filter`,
`enabled_update_charts_recursive`, `force_full`, optional time override).
`UpdateContext::from_params_now_or_override` derives the actual update/query
context, resolving the update time once (`Utc::now()` unless overridden) and
adding a per-update `UpdateCache` (`UpdateCache`/`Cacheable`/`CacheValue`) so
multiple charts in the same group update can reuse an already-computed
sub-result without re-querying the indexer DB. The cache has no invalidation
logic because a fresh one is created per group update and dropped after.

`IndexerMigrations` (`data_source::types::IndexerMigrations`) tracks whether
the indexer DB has run specific migrations (currently `denormalization`),
queried once per update cycle from Blockscout's `migrations_status` table
(only in `Blockscout`/`Zetachain` mode; other modes get `IndexerMigrations::empty()`).
Some data sources branch their SQL on this flag.

### Indexing status gating

Location: `stats/src/charts/indexing_status.rs`

`IndexingStatus { blockscout: BlockscoutIndexingStatus, user_ops: UserOpsIndexingStatus, zetachain_cctx: ZetachainCctxIndexingStatus }`
is a per-axis "how much of the source data must be indexed before this chart
is safe to compute" requirement. Each axis implements `IndexingStatusTrait`
with `MIN`/`MAX`/`LEAST_RESTRICTIVE`/`MOST_RESTRICTIVE` and
`is_requirement_satisfied`. `ChartProperties::indexing_status_requirement()`
declares a chart's own requirement; `UpdateGroup::dependency_indexing_status_requirement`
takes the most restrictive requirement across a group's *enabled* members
(via `IndexingStatus::most_restrictive_from`). This feeds
`stats-server/src/blockscout_waiter.rs`'s conditional-start logic (waiting on
`STATS__BLOCKSCOUT_API_URL` indexing progress before starting updates).

## Runtime Wiring (stats-server)

### `RuntimeSetup`

Location: `stats-server/src/runtime_setup.rs`

Built once at startup from `charts.json` + `layout.json` + `update_groups.json`
(`RuntimeSetup::new`). Holds:

- `charts_info: BTreeMap<String, EnabledChartEntry>` — per-chart-name enabled
  settings + per-resolution dynamic chart info (`ChartTypeSpecifics`)
- `update_groups: BTreeMap<String, UpdateGroupEntry>` — each group's
  `SyncUpdateGroup` handle, its configured cron schedule, and the subset of
  its members that are actually enabled by config
- `lines_layout` / `counters_layout` — API response ordering from `layout.json`

An entry's chart id can be served by a different registered chart via the
optional `implementation` field in `charts.json` (`AllChartSettings`) — the
public id keeps its own title/description while the data comes from the
named implementation (used by `STATS__ENABLE_ALL_FILECOIN`, which remaps the
public `txnsFee` id onto `filecoinNewChainFees`; see
`stats-server/src/settings.rs::handle_enable_all_filecoin`).

### `UpdateService`

Location: `stats-server/src/update_service.rs`, `update_tracker.rs`

Owns the DB connections and `RuntimeSetup`, runs each enabled update group on
its configured (or default) cron `Schedule`, tracks per-chart "initial
update" completion (`InitialUpdateTracker`), and exposes an on-demand
reupdate channel (`OnDemandReupdateRequest`) that `ReadService` can use to
trigger an eager update on a stale chart's query path.

### `ReadService`

Location: `stats-server/src/read_service.rs`

Implements the generated `StatsService` gRPC trait (served over both gRPC and
REST via `actix-prost` route generation — see `stats-proto`). Resolves a
chart id + resolution to its dynamic `QuerySerializedDyn` handle
(`stats/src/charts/query_dispatch.rs`), applies request range /
`RequestedPointsLimit` checks, and — when `STATS__LINKED_STATS__BASE_URL` is
configured — merges in data from a secondary linked stats deployment via
`linked_stats.rs`/`linked_stats_merge.rs` to fill gaps (bounded by
`LINKED_STATS_MAX_HOPS_HARD_CAP` to prevent forwarding loops across chained
linked services).

## Database Schema (stats DB)

```text
charts (id, name, resolution, chart_type, last_updated_at, ...)
    ↑
chart_data (chart_id, date, value, min_blockscout_block, ...)
```

Defined by the `entity`/`migration` crates (`stats/stats/entity`,
`stats/stats/migration`). This is the service's *own* database — distinct
from the indexer database(s) it reads from.

## Query Dispatch

Location: `stats/src/charts/query_dispatch.rs`

Bridges the statically-typed `DataSource`/`ChartProperties` world to the
dynamically-typed world `RuntimeSetup`/`ReadService` need (a
`Vec<ChartObject>` of heterogeneous chart types). `QuerySerialized`/
`QuerySerializedDyn`/`ChartTypeSpecifics` erase each chart's concrete type
behind `CounterHandle`/`LineHandle` trait objects while preserving whether it
is a line chart or counter (`ChartObject::construct_from_chart` asserts the
erased type matches `ChartProperties::chart_type()`).
