# Gotchas

Non-obvious traps and their solutions. Each entry was verified against the
current source, not inferred from naming alone.

## Enabling a Dependency Without Making It an Update-Group Member Does Nothing

**Symptom:** A chart that only exists as a dependency of another chart (e.g.
enabled in `charts.json` on its own) never gets recomputed, even though it is
"enabled".

**Root cause:** `construct_update_group!` (`stats/src/update_group.rs`) only
recurses into the members literally listed in a group's `charts: [...]`. If
chart `A` depends on `B` but the group only lists `A`, then: `A` on + `B` on
→ fine (triggered via `A`); `A` on + `B` off → fine; `A` off + `B` on → `B` is
never triggered, because the group has no member to recurse from — even
though `B` itself is "enabled". `A` off + `B` off → fine (nothing happens, as
expected).

**Fix:** Always include a chart's dependencies (and their dependencies) as
explicit group members, per the recommendation in the module doc comment of
`stats/src/update_group.rs` and `stats/src/update_groups.rs`. When a
dependency is intentionally never meant to be independently enabled (e.g. it
has no public chart id), list it as a member anyway — see the Filecoin
intermediate-chart entry below for the accepted pattern.

---

## `just test` Runs Ignored DB Tests, and the `justfile` Silently Overrides `DATABASE_URL`

**Symptom:** Running `just test` fails (or hangs) with DB connection errors,
or interferes with a locally running dev Postgres, even for changes that
don't look DB-related.

**Root cause:** `stats/justfile` computes `DATABASE_URL` from `DB_HOST`/`DB_PORT`
(defaulting to `localhost:5432`) as a top-level `export`, which applies to
every recipe and overrides any `DATABASE_URL` already set in the calling
shell. Additionally, the `test` recipe is `cargo test {{args}} -- --include-ignored --nocapture`
— it always attempts to run `#[ignore = "needs database to run"]` tests, which
make up most of `stats/src/charts/**` tests.

**Fix:** Prefer `just test-with-db`, which brings up a disposable Postgres on
`TEST_DB_PORT` (default `9439`) via `start-postgres-and-build-tests` and then
runs `just test` against it with `db-port`/`db-name` overridden — it's the
only self-contained, non-interfering full run. For a single non-DB test,
`just test <test_name>` is fine. Don't invoke bare `just test` expecting an
externally-set `DATABASE_URL` to be honored — it will be recomputed and
exported regardless.

---

## Disabling a Chart Is Keyed by What It *Serves*, Not Its Config Entry Name

**Symptom:** `STATS__DISABLE_INTERNAL_TRANSACTIONS=true` disables a
`charts.json` entry whose own name doesn't look internal-transactions-related.

**Root cause:** A `charts.json` entry can serve a *different* registered
chart's data via the optional `implementation` field (`AllChartSettings`).
`handle_disable_internal_transactions` (`stats-server/src/settings.rs`)
resolves `served_chart = settings.implementation.as_deref().unwrap_or(name)`
before checking whether the served chart needs
`BlockscoutIndexingStatus::InternalTransactionsIndexed` — so an entry is
disabled based on what it actually computes, not what its config key is
named. The reverse also holds: an entry whose *name* collides with a
dependent chart, but whose `implementation` points elsewhere, is correctly
left enabled. See `disable_internal_transactions_follows_implementation_remap`
in `stats-server/src/settings.rs` tests for the exact case.

**Fix:** When tracing why a config entry got disabled (or stayed enabled),
check its `implementation` field first, not just its config key.

---

## `enable_all_*` Convenience Flags Silently No-Op Outside Their Mode

**Symptom:** Setting `STATS__ENABLE_ALL_ARBITRUM=true` (or
`_OP_STACK`/`_EIP_7702`/`_FILECOIN`) produces no visible error, but the
expected charts never show up in the API.

**Root cause:** `enable_charts` (`stats-server/src/settings.rs`) looks up
each target chart id in the *currently loaded* `charts.json`/`layout.json`
config (`config/<mode>/`). If the id isn't present — e.g. because a
different mode's config is loaded, or a custom `charts.json` omits that
entry — the miss is logged at `warn!` with "This should not be a problem for
running the service," not surfaced as a startup error.

**Fix:** Confirm the loaded `charts_config`/`layout_config` actually contains
the target chart ids before assuming the flag is broken; check logs at
`warn` level for "chart not found in settings".

---

## The Second (CCTX) Indexer DB Only Connects in `Zetachain` Mode

**Symptom:** `STATS__SECOND_INDEXER_DB_URL` is set, but the service never
connects to it, and `db_choice::UseZetachainCctxDB` statements have no
connection to run against.

**Root cause:** `connect_to_second_indexer_db` (`stats-server/src/server.rs`)
only attempts the connection when `settings.mode == Mode::Zetachain`; every
other mode gets `None` unconditionally, regardless of whether the env var is
set. There's no warning for "second DB URL configured but ignored."

**Fix:** The second indexer DB is a `Zetachain`-mode-only concept by design.
If a chart needs a second DB connection in another mode, that requires new
wiring in `server.rs`/`UpdateParameters`, not just setting the env var.

---

## `IndexerMigrations` Is Only Ever Queried For Blockscout/Zetachain Mode

**Symptom:** A raw-SQL data source that branches on
`cx.indexer_applied_migrations.denormalization` always takes the
"not denormalized" branch in `MultichainAggregator`/`Interchain` mode, even
though the actual indexer schema may be fully migrated.

**Root cause:** `IndexerMigrations::query_from_db` (`stats/src/data_source/types.rs`)
only queries Blockscout's `migrations_status` table for
`Mode::Blockscout | Mode::Zetachain`; every other mode gets
`IndexerMigrations::empty()` unconditionally (see the `match` at the top of
`query_from_db`) — there's no equivalent migrations-status table to query in
those schemas.

**Fix:** Don't add a `denormalization`-gated code path to a
Multichain/Interchain-mode chart expecting it to ever see `true`; a new
migration flag for a non-Blockscout schema needs its own branch added to
`IndexerMigrations::query_from_db` and `IndexerMigrations::set`.

---

## Some Update-Group Members Are Deliberately Never Public Chart Ids

**Symptom:** `FilecoinChainFeesGroup` lists `BurnActorBalance`, `FevmFeeTips`,
and `FilecoinNewChainFees` as members, but none of them ever appear as a
selectable chart id in the API, even with `STATS__ENABLE_ALL_FILECOIN=true`.

**Root cause:** This is intentional, not an oversight. Per the comment above
`FilecoinChainFeesGroup` in `stats/src/update_groups.rs`, these "intermediate"
charts are listed as members purely to (a) silence the "Group has
dependencies that are not members" startup warning and (b) allow them to be
scheduled — but they stay `enabled: false` in `charts.json` forever.
`handle_enable_all_filecoin` (`stats-server/src/settings.rs`) only enables
`filecoinChainFeesGrowth` and (by remap) the public `txnsFee` id;
`filecoinNewChainFees` is explicitly never exposed as a public chart id, per
its own doc comment.

**Fix:** Don't "fix" a Filecoin intermediate chart's `enabled: false` as a
bug — check whether a public chart already serves its data via
`implementation` before assuming it's missing from the API.

---

## Trailing Line-Chart Points Are Marked Approximate By Design

**Symptom:** The most recent day (or two) of a line chart's data comes back
with `is_approximate: true` in tests or API responses, even though the value
itself looks complete.

**Root cause:** `ChartProperties::approximate_trailing_points()`
(`stats/src/charts/chart.rs`) defaults to `1` for line charts (`0` for
counters, since a counter only has one value): the current timespan isn't
finished yet as of the update time, so its data is necessarily partial (e.g.
"blocks today" is still growing). There's a documented edge case: if the
update time lands exactly at the start of a timespan (midnight), one fewer
trailing point is approximate, because a full timespan's data is already
available.

**Fix:** Don't treat a trailing `is_approximate: true` point as a computation
bug; it's expected for the most recent, still-in-progress timespan(s). If a
chart genuinely needs zero approximate trailing points (e.g. because it's
already a lagging aggregate), override `approximate_trailing_points()`
explicitly rather than working around the default elsewhere.

---

## Session Timezone Is Always UTC, So `TIMESTAMPTZ`/`TIMESTAMP` Casts Can't Drift

**Symptom:** A raw-SQL chart casts an indexer column with
`init_timestamp::date` / `::timestamp` (see
`stats/src/charts/lines/interchain/*.rs` and
`stats/src/charts/db_interaction/read/interchain.rs`). Those casts are
session-timezone-dependent when the underlying column is `TIMESTAMPTZ`, which
looks like it makes day bucketing depend on where the service (or CI, or a
developer's Postgres) runs.

**Root cause / why it can't actually drift:** every connection SeaORM opens
goes through `sqlx-postgres`, which pins `("TimeZone", "UTC")` in the startup
packet (`sqlx-postgres-0.8.6/src/connection/establish.rs:33`, alongside
`DateStyle=ISO, MDY` and `client_encoding=UTF8`). A server-level
`timezone = …` in `postgresql.conf` (or `docker run … -c timezone=…`) is
therefore overridden per session and never reaches a query. Verified by
running the interchain suite against a `postgres:17.5` started with
`-c timezone=Pacific/Kiritimati` (UTC+14): all 40 tests produce the same
values as against a UTC server.

**Fix:** Don't add defensive `AT TIME ZONE 'UTC'` wrappers to indexer-DB
queries, and don't treat a `TIMESTAMPTZ` → `TIMESTAMP` schema change as a
bucketing risk — the round trip through a pinned-UTC session cancels. Do note
the flip side: a naive `NaiveDateTime` bound to a `TIMESTAMPTZ` column is
interpreted as UTC, so fixtures and production data must genuinely mean UTC.
If you ever need a non-UTC session, it has to be set explicitly per
connection; it will not come from the server config.

---

## The Env-Docs Generator Does *Not* Derive Descriptions From Rust Doc Comments

**Symptom:** You add a settings field (or a whole nested settings struct) with
a carefully worded doc comment, run `just generate-envs`, and the new
`README.md` rows appear with an **empty description column**. Rewriting the
doc comment and regenerating changes nothing.

**Root cause:** `env-docs-generation` (`env-docs-generation/src/main.rs`) drives
`env_collector` from `blockscout-service-launcher`, which reflects over
`Settings` only for the *variable name* and the *default value* — it
serializes `Settings::default()`, it does not read source. The description
column is human-authored prose that the generator **preserves** across runs by
matching on the variable name. That is why
`STATS__INTERCHAIN_PRIMARY_ID`'s README description has never matched its Rust
doc comment. `just check-envs` validates names and defaults only, so a row with
an empty description passes validation.

**Fix:** Write the description directly into the `README.md` table cell, then
run `just generate-envs` (to normalise column padding) and `just check-envs`.
The doc comment is still worth writing — it is what a developer reading
`settings.rs` sees — but it is a *separate* artifact from the README row, and
the two have to be kept in sync by hand. Keep the Rust doc comment and the
README cell saying the same thing.

## `Expr::col(Column::X)` Renders **Unqualified**; `ColumnTrait` Methods Don't

**Symptom:** A SeaORM query that joins `crosschain_transfers` to
`crosschain_messages` compiles, but Postgres rejects it with
`column reference "bridge_id" is ambiguous` — or worse, it *runs* and silently
reads the wrong table's column.

**Root cause:** `sea_query::Expr::col(Column::X)` renders the bare identifier
(`"bridge_id"`), because a `Column` alone carries no table. Every `ColumnTrait`
method — `eq`, `is_in`, `is_not_null`, `lt`, `into_expr`, … — goes through
`as_column_ref()` and renders the qualified pair
(`"crosschain_transfers"."bridge_id"`). On a single-table query the two are
indistinguishable, which is why the mistake survives review.

This bites hardest on the interchain transfer charts: `crosschain_messages` and
`crosschain_transfers` **both** declare `id`, `bridge_id`, `created_at`,
`updated_at`, `sender_address` and `recipient_address`, and every transfer chart
joins the two. `interchain-indexer-filters` sidesteps it by building transfer
predicates from `Expr::col((Entity, Column))` *tuples*, which do qualify.

**Fix:** In any query that joins, reach a column through a `ColumnTrait` method
or `Column::X.into_expr()`, never `Expr::col(Column::X)`. If you genuinely need
`Expr::col`, pass the `(Entity, Column)` tuple. `Expr::col(Alias::new("date"))`
is the sanctioned exception — it names a `SELECT` output alias in `GROUP BY`,
not a table column. `charts::interchain_filter_coverage`'s
`transfer_predicates_are_table_qualified` asserts the qualification survives
future revisions of the shared filter crate.
