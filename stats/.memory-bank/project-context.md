# Project Context

## Purpose

`stats` is a standalone Rust microservice that calculates and serves statistical
information (charts and counters) derived from Blockscout-family data. It
connects to one or more source ("indexer") databases, periodically recomputes
a configured set of charts on a schedule, stores the computed points in its own
Postgres database, and serves them over a REST/gRPC API.

From `stats/README.md`:

> **Stats (Statistics)** - is a service designed to calculate and present
> statistical information from a Blockscout instance. This service establishes
> a connection with the Blockscout database and periodically updates a
> collection of charts, including lines and counters, based on a predefined
> schedule. The calculated data is then made available through a REST API.

## Modes

The service has one runtime mode, set via `STATS__MODE` (`stats/stats/src/mode.rs`,
`enum Mode`). Modes are mutually exclusive and determine which indexer database
schema is queried, which config files are used by default, and which
mode-specific charts get enabled:

- `Blockscout` (default) — single Blockscout instance. Indexer DB is the
  Blockscout database. Default config: `config/blockscout_instance/`.
- `MultichainAggregator` — reads from a `multichain-aggregator` indexer schema
  (crate `multichain-aggregator-entity`/`multichain-aggregator-migration`,
  pulled from the main blockscout-rs repo). Default config: `config/multichain/`.
  `STATS__MULTICHAIN_FILTER` restricts which chain IDs are included.
- `Zetachain` — a Blockscout instance plus a second "CCTX" (cross-chain
  transactions) indexer database (crate `zetachain-cctx-entity`/
  `zetachain-cctx-migration`, set via `STATS__SECOND_INDEXER_DB_URL`). Reuses
  `config/blockscout_instance/` (the zetachain-specific counters/lines already
  exist in that config, disabled by default) and is force-enabled via
  `apply_zetachain_cctx_mode_settings` (`stats-server/src/settings.rs`).
- `Interchain` — reads from the `interchain-indexer` (a.k.a. Universal Bridge
  Indexer) schema. Default config: `config/interchain/`.
  `STATS__INTERCHAIN_PRIMARY_ID` optionally centers send/receive charts around
  one chain.

Mode-specific settings adjustments (disabling Blockscout-only conditional-start
checks, clearing `blockscout_api_url`, etc.) live in
`stats-server/src/settings.rs`: `apply_multichain_mode_settings`,
`apply_interchain_mode_settings`, `apply_zetachain_cctx_mode_settings`. Mode
dispatch at startup is in `stats-server/src/server.rs` (`stats()` function,
`match settings.mode { ... }`).

## Crate Map

Cargo workspace members (`stats/Cargo.toml`):

- `stats` (path `stats/stats`)
  - the chart calculation library: data sources, chart/counter definitions,
    update groups, SQL statement builders. No networking; used by
    `stats-server`.
  - primary entrypoints:
    - `src/lib.rs`
    - `src/mode.rs` — `Mode` enum
    - `src/update_group.rs` — `UpdateGroup` trait, `construct_update_group!` macro, `SyncUpdateGroup`
    - `src/update_groups.rs`, `src/update_groups_interchain.rs`, `src/update_groups_multichain.rs`
    - `src/data_source/` — the `DataSource` trait and its `kinds::{local_db, remote_db, data_manipulation, auxiliary}`
    - `src/charts/` — `ChartProperties`, counters (`src/charts/counters/`), line charts (`src/charts/lines/`), read helpers (`src/charts/db_interaction/read/`)
- `stats-server` (path `stats/stats-server`)
  - the transport layer: settings/config loading, DB connections, HTTP/gRPC
    routing, the update scheduler, and the read API.
  - primary entrypoints:
    - `src/main.rs`
    - `src/server.rs` — startup wiring (`stats()` function)
    - `src/settings.rs` — `Settings`, `Mode` re-export, mode-specific setting adjustments
    - `src/runtime_setup.rs` — `RuntimeSetup`, wires configured charts to update groups and mutexes
    - `src/read_service.rs` — `ReadService`, implements the gRPC `StatsService` (serves both REST via `actix-prost` and gRPC)
    - `src/update_service.rs` — `UpdateService`, the update scheduler
    - `src/config/` — config file (JSON) and env-override loading (`env/`, `json/`, `read/`, `types.rs`)
- `entity` (path `stats/stats/entity`)
  - SeaORM entities for the stats service's own database (charts, chart_data, etc.)
- `migration` (path `stats/stats/migration`)
  - SeaORM migrations for the stats database schema
- `stats-proto` (path `stats/stats-proto`)
  - protobuf definitions (`proto/`) and generated Rust API bindings for the
    stats gRPC/REST service
- `env-docs-generation` (path `stats/env-docs-generation`)
  - a small binary (`env_collector` based) that generates/validates the
    environment-variable documentation table in `stats/README.md` from
    `Settings`; run via `just check-envs` / `just generate-envs`

Not a Cargo crate: `stats/types/` is a small **TypeScript** package
(`@blockscout/stats-types`, see `stats/types/package.json`) that compiles
`stats-proto`'s `.proto` into TypeScript type declarations for frontend
consumers. It has no `src/` and is unrelated to the Rust build.

## Main Runtime Components

- `Settings` — env + config-derived runtime configuration
  - source: `stats-server/src/settings.rs`
- `RuntimeSetup` — combines the charts config, layout config, and update
  groups config into the live set of `SyncUpdateGroup`s and per-chart mutexes
  - source: `stats-server/src/runtime_setup.rs`
- `UpdateService` — schedules and runs recurring/one-off chart updates against
  update groups, tracks initial-update state
  - source: `stats-server/src/update_service.rs`, `stats-server/src/update_tracker.rs`
- `ReadService` — implements the `StatsService` gRPC/REST surface, applies
  request-time chart lookups, linked-stats gap-filling, and authorization
  - source: `stats-server/src/read_service.rs`
- `DataSource` (trait) — the recursive init/update/query contract shared by
  every chart and its dependencies
  - source: `stats/src/data_source/source.rs`, `stats/src/data_source/types.rs`
- `SyncUpdateGroup` — synchronizes updates of a DAG of related charts using
  per-chart mutexes, in lexicographic lock order to avoid deadlocks
  - source: `stats/src/update_group.rs`

## External Systems

- PostgreSQL (stats DB)
  - the service's own database storing computed chart points
    (`entity`/`migration` crates)
- Indexer database(s)
  - the data source charts are computed from; schema depends on `Mode`
    (Blockscout DB, `multichain-aggregator` DB, `interchain-indexer` DB, plus
    an optional second "CCTX" DB in `Zetachain` mode)
- Optional Blockscout API
  - `STATS__BLOCKSCOUT_API_URL`, used for conditional start checks (waiting
    for indexing progress) — see `stats-server/src/blockscout_waiter.rs`
- Optional linked secondary stats service
  - `STATS__LINKED_STATS__BASE_URL`; used by `ReadService` to fill gaps in
    read responses from another stats deployment — see
    `stats-server/src/linked_stats.rs`, `linked_stats_merge.rs`

## Configuration Model

### Static Repo Configuration

Config files live under `config/<mode-dir>/`:

- `config/blockscout_instance/{charts,layout,update_groups}.json` — default
  for `Blockscout` and `Zetachain` modes
- `config/multichain/{charts,layout,update_groups}.json` — default for
  `MultichainAggregator` mode
- `config/interchain/{charts,layout,update_groups}.json` — default for
  `Interchain` mode

`charts.json` enables/disables and titles individual charts (and can remap a
public chart id onto a different registered implementation via
`implementation`); `layout.json` controls ordering/grouping in API responses;
`update_groups.json` sets cron schedules per update group. Loading and
env-overriding of these files is implemented in `stats-server/src/config/`
(`json/`, `env/`, `read/`).

### Runtime Configuration

- env prefix: `STATS__`
- config assembly source: `stats-server/src/settings.rs`
- generated documentation: the env var table in `stats/README.md`, kept in
  sync via `just check-envs` (validate) / `just generate-envs` (regenerate),
  implemented by the `env-docs-generation` binary

Notable settings: `mode`, `db_url`, `indexer_db_url` (+ deprecated
`blockscout_db_url`), `second_indexer_db_url`, `multichain_filter`,
`interchain_primary_id`, `charts_config`/`layout_config`/`update_groups_config`
paths, `enable_all_arbitrum`/`enable_all_op_stack`/`enable_all_eip_7702`/
`enable_all_filecoin` convenience flags, `disable_internal_transactions`,
`conditional_start` thresholds, `linked_stats`, `limits.requested_points_limit`.

## Local Development Flow

Primary task runner: `just` (see `stats/justfile`)

Common commands:

- `just` — list available recipes
- `just start-postgres` — run a disposable local Postgres container (stats DB)
- `just start-test-postgres` / `just stop-test-postgres` — disposable Postgres
  for tests, on `TEST_DB_PORT` (default `9439`)
- `just run-multichain` — run the server in `MultichainAggregator` mode against
  `config/multichain/` with sample env values
- `just run-interchain` — run the server in `Interchain` mode against
  `config/interchain/`
- `just format` — `cargo sort --workspace` (if installed) + `cargo fmt --all`
- `just check` — `cargo check` + `cargo clippy --all --all-targets --all-features -- -D warnings`
- `just format-check` — `just format` then `just check`
- `just check-envs` — validates that `stats/README.md`'s env docs are in sync
  with `Settings` (`cargo run --bin env-docs-generation -- --validate-only`)
- `just generate-envs` — regenerates the env docs table in `stats/README.md`
- `just restart-generate-entities` — bring up a fresh Postgres, run migrations,
  regenerate SeaORM entities (`stats/migrate-up`, `stats/generate-entities`
  recipes, invoked from the `stats/stats` crate's own justfile)

## Testing Flow

- `just test *args` — `cargo test {{args}} -- --include-ignored --nocapture`
- `just test-with-db *args` — starts a disposable test Postgres
  (`start-postgres-and-build-tests`), then runs `just test` against it with
  `db-port`/`db-name` overridden; tears the container down implicitly via the
  `-just stop-test-postgres` cleanup at the start of the next run
- Most DB-backed chart tests are `#[ignore = "needs database to run"]` and use
  `stats/src/tests/` helpers (`init_db_all`, `fill_mock_blockscout_data`,
  `simple_test_chart`, `get_counter`, `test_counter_fallback`, etc.)
- Server-level integration tests live in `stats-server/tests/it/`
  (`mock_blockscout_simple/`, `mock_blockscout_reindex.rs`, `linked_stats.rs`)

## Current Constraints

- Mode-specific behavior is scattered across `stats-server/src/settings.rs`
  (the `apply_*_mode_settings`/`handle_*` functions) and
  `stats/src/data_source/kinds/remote_db/db_choice.rs`
  (`UsePrimaryDB`/`UseZetachainCctxDB`) — adding a new indexer schema variant
  touches both layers plus new `data_source::kinds::remote_db` statement
  types.
- Update groups intentionally batch charts by shared, expensive dependency
  queries (see doc comments in `stats/src/update_groups.rs`); adding a chart
  outside its natural group risks either missing dependency triggers or an
  update-mutex deadlock — see `.memory-bank/gotchas.md`.
- The env var documentation in `stats/README.md` is generated, not
  hand-maintained; `just check-envs` must pass (or `just generate-envs` +
  review) whenever `Settings` gains/changes a field.
