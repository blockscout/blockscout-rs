# Exploration Map

## If You Need to Understand the Whole System

- `.memory-bank/project-context.md`
  - high-level repo scope, crate responsibilities, runtime components, and local workflow
- `.memory-bank/architecture.md`
  - chart/data-source framework, update groups, runtime wiring
- `stats/README.md`
  - service description, config model, env var reference table
- `stats-server/src/main.rs`
  - process entrypoint
- `stats-server/src/server.rs`
  - startup wiring: config loading, mode dispatch, DB connections, service construction, launch
- then continue to:
  - `stats-server/src/runtime_setup.rs`
  - `stats-server/src/update_service.rs`
  - `stats-server/src/read_service.rs`

## If You Need to Understand a Specific Mode (Blockscout / MultichainAggregator / Zetachain / Interchain)

- `stats/src/mode.rs`
  - the `Mode` enum
- `stats-server/src/settings.rs`
  - `apply_multichain_mode_settings`, `apply_interchain_mode_settings`,
    `apply_zetachain_cctx_mode_settings` — mode-specific settings/chart-enablement adjustments
- `stats-server/src/server.rs`
  - `match settings.mode { ... }` dispatch at startup, `connect_to_main_indexer_db`,
    `connect_to_second_indexer_db`
- `config/blockscout_instance/`, `config/multichain/`, `config/interchain/`
  - the per-mode default `charts.json` / `layout.json` / `update_groups.json`
- `stats/src/data_source/kinds/remote_db/db_choice.rs`
  - `UsePrimaryDB` / `UseZetachainCctxDB` — which indexer connection a raw-SQL data source targets
- then continue to:
  - `stats/src/charts/counters/interchain/`, `stats/src/charts/counters/multichain/`,
    `stats/src/charts/lines/interchain/`, `stats/src/charts/lines/multichain/` for mode-specific charts

## If You Need to Add or Modify a Chart

- `stats-server/src/runtime_setup.rs`
  - doc comment "Adding new charts" at the top of the file spells out the steps
- `stats/src/charts/lines/blockscout_instance/blocks/new_blocks.rs`
  - a representative line-chart example: `RemoteDatabaseSource` + `StatementFromRange`,
    `ChartProperties`, `define_and_impl_resolution_properties!` for week/month/year variants,
    `#[cfg(test)]` with `simple_test_chart`
- `stats/src/charts/counters/blockscout_instance/total_blocks.rs`
  - a representative counter example: `RemoteQueryBehaviour`, `ValueEstimation` fallback,
    `DirectPointLocalDbChartSourceWithEstimate`
- `stats/src/charts/chart.rs`
  - `ChartProperties`, `ChartKey`, `ChartObject`, `define_and_impl_resolution_properties!`
- `stats/src/update_group.rs`, `stats/src/update_groups.rs`
  - where the new chart's update group is declared/extended
- `config/<mode>/charts.json`, `config/<mode>/layout.json`, `config/<mode>/update_groups.json`
  - where the new chart is enabled, placed in layout, and scheduled
- then continue to:
  - `.memory-bank/gotchas.md` — update-group membership and dependency pitfalls

## If You Need to Understand the `DataSource` / `kinds` Framework

- `stats/src/data_source/mod.rs`
  - module-level doc comment: overview, composability rationale, usage model
- `stats/src/data_source/source.rs`
  - the `DataSource` trait itself
- `stats/src/data_source/types.rs`
  - `UpdateContext`, `UpdateParameters`, `UpdateCache`, `IndexerMigrations`
- `stats/src/data_source/kinds/remote_db/`
  - `RemoteDatabaseSource`, `RemoteQueryBehaviour`, `StatementFromRange`/`StatementForOne`/
    `StatementFromTimespan`/`StatementFromUpdateTime`, `db_choice.rs`
- `stats/src/data_source/kinds/local_db/`
  - `LocalDbChartSource` and its `Direct*`/`DailyCumulative*` aliases, `parameter_traits.rs`,
    `parameters/` (create/update behaviours, batching)
- `stats/src/data_source/kinds/data_manipulation/`
  - `map/`, `delta.rs`, `sum_point.rs`, `last_point.rs`, `filter_deducible.rs`, `resolutions/`
- `stats/src/preludes.rs`
  - `chart_prelude` — the single import most chart implementation files use
    (`use crate::chart_prelude::*;`)
- then continue to:
  - `stats/src/data_source/tests.rs` for worked examples of composing data sources

## If You Need to Understand Update Scheduling / Synchronization

- `stats/src/update_group.rs`
  - `UpdateGroup` trait, `construct_update_group!` macro, `SyncUpdateGroup` (mutex lock ordering)
- `stats-server/src/runtime_setup.rs`
  - `UpdateGroupEntry`, how groups + their enabled members + schedules are assembled
- `stats-server/src/update_service.rs`
  - `UpdateService`, cron-driven update loop, on-demand reupdate channel
- `stats-server/src/update_tracker.rs`
  - `InitialUpdateTracker` — tracks first-run completion per chart
- then continue to:
  - `config/<mode>/update_groups.json` for the actual cron schedules

## If You Need to Understand the Read API

- `stats-server/src/read_service.rs`
  - `ReadService`, implements the generated `StatsService` trait
- `stats/src/charts/query_dispatch.rs`
  - `QuerySerialized`/`QuerySerializedDyn`/`ChartTypeSpecifics`/`CounterHandle`/`LineHandle`
- `stats-server/src/linked_stats.rs`, `stats-server/src/linked_stats_merge.rs`
  - gap-filling from a secondary linked stats deployment (`STATS__LINKED_STATS__*`)
- `stats-server/src/auth.rs`
  - `AuthorizationProvider`, `STATS__API_KEYS__<KEY_NAME>`
- `stats-proto/proto/`
  - the protobuf/REST contract
- then continue to:
  - `stats-server/tests/it/mock_blockscout_simple/` for end-to-end read API test examples

## If You Need to Understand Config Loading

- `stats-server/src/settings.rs`
  - `Settings`, `LimitsSettings`, `StartConditionSettings`, mode-adjustment functions
- `stats-server/src/config/`
  - `json/` (raw JSON shapes for `charts.json`/`layout.json`/`update_groups.json`),
    `env/` (env-var override layer per config file), `read/` (loading + merge), `types.rs`
    (`AllChartSettings`, `EnabledChartSettings`, `LineChartCategory`)
- `config/blockscout_instance/`, `config/multichain/`, `config/interchain/`
  - the actual default config files
- `env-docs-generation/src/main.rs`
  - drives `just check-envs` / `just generate-envs`; defines the env-var filters/sections used
    to regenerate the table in `stats/README.md`
- then continue to:
  - `stats/README.md` → "Config" section

## If You Need to Understand Testing Helpers / Mock Data

- `stats/src/tests/simple_test.rs`
  - `simple_test_chart`, `get_counter`, `test_counter_fallback`
- `stats/src/tests/init_db.rs`
  - `init_db_all`, `init_db_all_interchain`, `init_db_all_multichain`, `init_db_zetachain_cctx`
- `stats/src/tests/mock_blockscout.rs`, `mock_blockscout_filecoin.rs`, `mock_interchain.rs`,
  `mock_multichain.rs`, `mock_zetachain_cctx.rs`
  - per-mode/per-feature mock indexer data fixtures
- `stats/src/tests/point_construction.rs`
  - helpers for building expected `TimespanValue`/`DateValue` test data
- `stats-server/tests/it/`
  - `common.rs`, `mock_blockscout_simple/` (full server integration tests), `linked_stats.rs`,
    `mock_blockscout_reindex.rs`

## If You Need to Understand Raw SQL Statement Building

- `stats/src/utils.rs`
  - `sql_with_range_filter_opt!` / `sql_with_multichain_filter_opt!` macros,
    `produce_filter_and_values`, `produce_day_filter_and_values` — all build a
    `sea_orm::Statement::from_sql_and_values(...)` with placeholder-safe value interpolation
- `stats/src/charts/db_interaction/read/mod.rs`
  - `ReadError`, `QueryFullIndexerTimestampRange`, `get_min_date` (mode-dispatching to
    `blockscout`/`interchain`/`multichain` variants)
- `stats/src/charts/lines/blockscout_instance/blocks/new_blocks.rs`
  - a concrete `sql_with_range_filter_opt!` usage example
