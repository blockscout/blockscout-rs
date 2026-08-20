# Glossary

## Chart

Informally, anything that implements both `DataSource` (update/query
behavior) and `ChartProperties` (static metadata: name, resolution, chart
type) and is stored via a `data_source::kinds::local_db` type. See
`stats/src/charts/chart.rs` module doc comment.

## Counter

A chart whose `ChartProperties::chart_type()` is `ChartType::Counter` — a
single current value (e.g. "total blocks"), not a time series. Typically
built with `DirectPointLocalDbChartSource[WithEstimate]`. Counters live under
`stats/src/charts/counters/`.

## Line Chart

A chart whose `chart_type()` is `ChartType::Line` — a time series of
`(timespan, value)` points (e.g. "new blocks per day"). Typically built with
`DirectVecLocalDbChartSource`. Line charts live under `stats/src/charts/lines/`.

## Resolution

The time granularity of a chart's `Resolution` associated type
(`stats::types::Timespan` impls: `NaiveDate` for daily, or `Week`/`Month`/`Year`).
`ChartKey { name, resolution }` uniquely identifies one resolution variant of
one chart (e.g. `newBlocks` at `Day` vs `Week`). Weekly/monthly/yearly
variants of a daily chart are usually generated via
`define_and_impl_resolution_properties!` plus a
`SumLowerResolution`/`AverageLowerResolution`/`LastValueLowerResolution`
wrapper (`data_source::kinds::data_manipulation::resolutions`).

## `DataSource`

The trait every node in the chart dependency DAG implements
(`stats/src/data_source/source.rs`): `init_recursively`,
`update_recursively`, `query_data`, each expected to act on dependencies
first, then on itself. See `architecture.md`.

## Update Group

A DAG of related charts that update together, constructed via
`construct_update_group!` (`stats/src/update_group.rs`) or the
`singleton_groups!` helper for standalone charts. Grouping lets charts that
share an expensive dependency reuse that dependency's single computed
result within one update, and keeps their computed data consistent as of the
same update timestamp. See `stats/src/update_groups.rs` module doc comment
for the reasoning and a worked "A depends on B" example of why dependencies
should be included as members.

## `SyncUpdateGroup`

The synchronized wrapper around an `UpdateGroup` (`stats/src/update_group.rs`)
that acquires per-chart mutexes (shared across all groups, one per
`DataSource::mutex_id()`) in lexicographic order before running
create/update/reset operations, to make concurrent group updates
deadlock-free.

## Data Source (raw-SQL sense)

Also used loosely for a `RemoteDatabaseSource<Behaviour>` — a `DataSource`
leaf that pulls data from an indexer database by executing a raw SQL
`Statement` built by a `RemoteQueryBehaviour`/`StatementFromRange`/etc. impl.
See `stats/src/data_source/kinds/remote_db/`.

## Mode

The service's single runtime mode (`stats::Mode`, `stats/src/mode.rs`):
`Blockscout`, `MultichainAggregator`, `Zetachain`, or `Interchain`. Set via
`STATS__MODE`; mutually exclusive. Determines the indexer DB schema, default
config directory, and which mode-specific charts/settings get enabled. See
`project-context.md` → "Modes".

## Indexer DB

The external database a chart's data actually comes from — distinct from the
stats service's own database. Its schema depends on `Mode`: the Blockscout
DB, the `multichain-aggregator` DB, or the `interchain-indexer` DB. Reached
via `UpdateContext::indexer_db` / `UpdateParameters::indexer_db`.
`Zetachain` mode additionally has a `second_indexer_db` (the CCTX indexer),
reached via `db_choice::UseZetachainCctxDB`.

## Stats DB

The service's own PostgreSQL database (crates `entity` + `migration`,
tables `charts` and `chart_data`), storing computed chart points. Distinct
from the indexer DB(s) charts are computed *from*.

## `ChartKey`

`{ name: String, resolution: ResolutionKind }` (`stats/src/charts/chart.rs`).
The combination is expected (not type-enforced) to be unique across all
registered charts; `ChartProperties::key()` builds it from `Self::name()` and
`Self::resolution()`.

## `UpdateContext` / `UpdateParameters`

`UpdateParameters` is the input bundle a caller assembles (DB connections,
mode, filters, `force_full`, optional time override).
`UpdateContext::from_params_now_or_override` derives the actual per-update
context used during a recursive update/query, resolving the update timestamp
once and attaching a fresh `UpdateCache`. See `architecture.md`.

## `IndexerMigrations`

A per-update-cycle record of which indexer-DB migrations are active
(currently just `denormalization`), queried once from Blockscout's
`migrations_status` table in `Blockscout`/`Zetachain` mode
(`data_source::types::IndexerMigrations::query_from_db`). Other modes get
`IndexerMigrations::empty()`. Some raw-SQL data sources branch their query on
this flag.

## `IndexingStatus`

A three-axis requirement (`blockscout`, `user_ops`, `zetachain_cctx`,
`stats/src/charts/indexing_status.rs`) describing how far the indexer must
have progressed before a chart is safe to compute. Each chart declares its
own via `ChartProperties::indexing_status_requirement()`; an update group's
overall requirement is the most restrictive across its currently-enabled
members. Drives the conditional-start wait in
`stats-server/src/blockscout_waiter.rs`.

## `implementation` (chart remap)

An optional field on a `charts.json` entry (`AllChartSettings`) that serves a
public chart id's data from a *different* registered chart's implementation,
while keeping the public id's own title/description. Used by
`STATS__ENABLE_ALL_FILECOIN` to serve the public `txnsFee` id with the
`filecoinNewChainFees` implementation. See
`stats-server/src/settings.rs::handle_enable_all_filecoin` and
`stats/README.md` → "Config files".

## Linked Stats

An optional secondary stats deployment (`STATS__LINKED_STATS__BASE_URL`) that
`ReadService` queries to fill gaps in its own read responses (e.g. history
this deployment doesn't have). Hop count is capped by
`LINKED_STATS_MAX_HOPS_HARD_CAP` to bound forwarding chains. See
`stats-server/src/linked_stats.rs`, `linked_stats_merge.rs`.

## `types/` (TypeScript package)

Not a Rust crate. `stats/types/` is `@blockscout/stats-types`, a small
TypeScript package that compiles `stats-proto`'s `.proto` definitions into
`.d.ts` type declarations for frontend consumers (`stats/types/package.json`).
