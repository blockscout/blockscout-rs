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

---

## In `Interchain` Mode, `chart_data.min_blockscout_block` Is Not a Block Number

**Symptom:** You read `chart_data.min_blockscout_block` on an interchain
deployment expecting a block height and get an arbitrary 63-bit integer. Or you
"generalise" the interchain clear-on-mismatch branch in
`update_itself_inner` to the other modes and every Blockscout chart starts
deleting its history on the first reindex.

**Root cause:** the interchain indexer has no block numbers, so that column is
reused as the *interchain filter fingerprint*
(`charts::db_interaction::filters::interchain::filter_fingerprint`, returned by
`read::interchain::get_min_block_interchain`). `last_accurate_point` already
compares the recorded value against the observed one on every update, so
stamping the fingerprint there turns that comparison into "was this history
computed under the currently configured filter?" for free — no migration, no
new column. In `Blockscout`/`Zetachain`/`MultichainAggregator` mode the column
still means an actual minimum indexed block and the reindex semantics depend on
it.

Two consequences that are easy to get wrong:

- The fingerprint hashes only the five operator-configured id lists **plus a
  boolean** for whether the observability-horizon restriction is enabled — never
  the horizon's resolved `(bridge, chain)` pairs. Those are DB-derived and grow
  on their own, so hashing them would rebuild all interchain series every time
  the indexer gained a bridge or a bridge contract. The flip side is that an
  upstream horizon change is **not** detected — `filter_fingerprint`'s own doc
  comment states the trade.
- The fingerprint is remapped away from `0` and `i64::MAX`. Masking to 63 bits
  makes `i64::MAX` *reachable*, and `i64::MAX` is exactly the sentinel every
  pre-existing interchain row carries, so an unlucky hash would silently look
  like "unchanged".

**Fix:** keep the clear gated on `cx.mode == Mode::Interchain`, and treat the
column as mode-dependent whenever you touch it.

---

## `insert_data_many` Is an Upsert With No Delete — Narrowing a Filter Corrupts History

**Symptom:** you narrow a read filter (interchain or multichain), the charts
recompute, and days that should now be empty keep their old non-zero values.
On a cumulative chart (`DailyCumulativeLocalDbChartSource`) the stale prefix
propagates through every later point, so the whole series is wrong, not just
those days.

**Root cause:** `write::insert_data_many` is `ON CONFLICT (chart_id, date) DO
UPDATE`. A recompute only ever *overwrites* days that still produce a row; a day
that now produces no row is simply never written, so the previous regime's value
survives. Forcing a full recompute (`force_full` /
`STATS__FORCE_UPDATE_ON_START=true`) does **not** help — it skips
`last_accurate_point`, not the missing delete. Widening is self-correcting;
narrowing is not.

**Fix:** an explicit delete before the recompute. Interchain does this
automatically on a fingerprint change (`LocalDbChartSource::update_itself_inner`)
via `write::clear_chart_data_and_updated_at`; `ClearAllAndPassVec` and the
windowed batch step are the other two places that delete, and they use plain
`write::clear_all_chart_data`.

The interchain clear is **not** atomic with the rebuild, and cannot be:
`BatchUpdate` commits per batch by design, and wrapping a full universal-indexer
backfill in one transaction trades this problem for a worse one. What it does
instead is clear `charts.last_updated_at` in the same transaction as the delete,
so the window is *honest* rather than absent:

- `BatchUpdate` calls `update_metadata` after **every** batch, so once the first
  batch commits, `last_updated_at` tracks the rebuild and the next run resumes
  from there. The chart serves a partially rebuilt series while the backfill runs
  — by design.
- Before the first batch commits, the chart has zero rows. Without the timestamp
  reset it would also still carry its **pre-clear** `last_updated_at` — i.e. read
  as "empty, and fresh as of yesterday", with nothing reporting it stale. With
  the reset it reads as never updated, which is what it is.

So: an interrupted interchain rebuild still serves an empty or partial series.
The guarantee is only that it no longer claims to be up to date while doing so.

---

## The Shared Filter's Permissive "Absent Bridge" Arm Is Unreachable From Stats

**Symptom:** you read `ChainBridgeFilter::messages_condition` in
`interchain-indexer-filters`, see the permissive arm
(`bridge_id NOT IN (listed) AND dst_chain_id IS NOT NULL`) that keeps a
decommissioned bridge's history visible, and assume stats inherits that
behaviour because it shares the predicate. It does not — and a bridge removed
from the indexer's configuration can therefore be visible through the indexer
API and partly missing from stats, even though both run the same code.

**Root cause:** the arm's reachability depends on the *input*, not the
predicate. The indexer builds `only_indexed_by_bridge` from its in-memory
`bridges.json`, so a removed bridge is genuinely absent from the pair list and
its rows take the permissive arm. Stats builds the same list in
`read::interchain::resolve_only_indexed_by_bridge` from `bridges` LEFT JOIN
`bridge_contracts`, and there:

- every `bridges` row survives the LEFT JOIN, contract-less ones included
  (that is the point of the outer join); and
- every `crosschain_messages.bridge_id` is a foreign key into `bridges`
  (`crosschain_messages::Relation::Bridges`).

So every row's bridge is always *listed*, `bridge_id NOT IN (listed)` is never
true, and the permissive disjunct is dead code. A removed bridge's rows are
still tested against the chain set recorded for it — `upsert_bridge_contracts`
never deletes, so that set is a superset of its last configured one — and any
row with an endpoint outside it is dropped.

Pruning to `STATS__INTERCHAIN_FILTER__BRIDGE_IDS` does not resurrect the arm
either: the pruned bridges are excluded anyway by the separate
`bridge_id IN (bridge_ids)` term in the outer AND.

There is a second, opposite divergence from the same cause. `upsert_bridges` and
`upsert_bridge_contracts` never delete, so removing a *contract* from a bridge
that still exists shrinks the indexer's set and leaves stats' a superset: stats
then admits rows the API excludes. Removing a *whole bridge* goes the other way,
as above. Both are in `README.md`'s "Parity, and its two known divergences".

**Fix:** none — these are the two named divergences in the parity claim. Closing it needs the
indexer to publish its effective `configured_pairs` (an API or a table), not a
stats-side workaround. Do not "fix" it by dropping contract-less bridges from
the resolver: that flips the contract-less-bridge case from restrictive to
permissive, which ADR-004 Decision 5 calls load-bearing in the other direction.

## "Directional" Interchain Charts Are Detected by a Substring Match on the Chart Id

`stats-server/src/interchain_filter.rs`'s startup validation warns when
`STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID` is unset while directional charts are
enabled. It finds those charts with

```rust
.filter(|name| name.contains("Sent") || name.contains("Received"))
```

over the enabled chart keys. Two consequences, both deliberate for now:

- **The warning fires on the documented default.** No home chain *is* the
  default, so a default interchain deployment logs a `warn!` listing eight chart
  ids at every startup. It is kept at `warn!` on purpose: the configuration it
  describes is genuinely ambiguous (those charts degrade to "source-side
  observed" / "destination-side observed" rather than a direction relative to any
  chain), and `info!` would bury that. Do not read this particular warning as a
  defect report.
- **It silently stops covering.** A future directional chart named
  `newMessagesOutboundInterchain` or `transfersFromHome` is missed, and a
  non-directional chart whose id happens to contain `Sent`/`Received` is falsely
  included. Nothing fails — the warning just becomes wrong.

"Does this chart's meaning depend on a focal chain?" is a fact about the chart,
so the structurally correct home for it is `ChartProperties` (next to
`indexing_status_requirement()`) or a const on the `InterchainFiltered` impls,
which already exist for all thirteen statements. That was considered and
**deliberately not done**: it means exporting a chart-key → directional mapping
from `stats` to `stats-server` for the sake of a startup log line. If you are
adding a directional interchain chart whose id carries neither word, either
rename it or do that refactor then — the substring list is not somewhere to add
a special case.

## Interchain Chart Titles *and Descriptions* in `charts.json` Are Frozen UI Strings

`config/interchain/charts.json`'s `title` and `description` are rendered in the
Blockscout UI. They are **product copy, not documentation**, and are not to be
rewritten as a side effect of a code change — even when the change makes them
less precise.

This is a live example. The read filter (`InterchainFiltered`) means the four
`total_*`/`new_*_interchain` charts no longer count every indexed row, so
descriptions like `"Total indexed inter-chain messages"` now overstate their
scope. A rewrite around *"within the configured chain and bridge scope"* was
made and then **deliberately reverted**: churning strings the UI already shows
costs more than the imprecision it fixes.

Where to put the caveat instead:

- **For operators of one deployment** — the per-chart env override, which layers
  on top of whatever the JSON ships and is mode-independent
  (`config/read/mod.rs:read_json_override_from_env_config`):
  `STATS_CHARTS__COUNTERS__<COUNTER_NAME>__DESCRIPTION`,
  `STATS_CHARTS__LINE_CHARTS__<LINE_CHART_NAME>__DESCRIPTION`.
- **For the semantics themselves** — `README.md`'s interchain operator section
  and `.memory-bank/research/interchain-mode-and-filtering.md`, which is where
  the six filter dimensions and the horizon are actually explained.

Note that the imprecision is not an open question hiding behind the strings.
"Should the `total_*` charts follow the filter, or keep describing the whole
DB?" was raised in the research note's §3.3 and **answered — all of them
follow it** (§11, question 2; a coverage test asserts it). So the descriptions
are plainly stale rather than debatable, and leaving them stale is a deliberate
UI-stability call. Changing them is a product decision, not a cleanup.

---

## A Transfer's Token Chains Are Not Its Message's Route

**Symptom:** you reason about interchain transfer charts using the parent
message's `src_chain_id` / `dst_chain_id` — for a filter, a scoping decision, or
a hand-written cross-check — and the numbers quietly disagree with what the
service produces.

**Root cause:** `crosschain_transfers` carries its **own**
`token_src_chain_id` / `token_dst_chain_id`, and they need not equal the parent
`crosschain_messages` row's `src_chain_id` / `dst_chain_id`. The indexer has
separate columns because the asset's canonical chains can differ from the
message's route (wrapped assets, reconstructed AMB transfers). The shared
`ChainBridgeFilter` mirrors that split deliberately, for parity with the
indexer's read API: `messages_condition` filters on the message's columns,
`transfers_condition` on the transfer's own. The composite join in
`InterchainFilter::transfers_joined_query` exists **only** to reach
`crosschain_messages.init_timestamp` (transfers have no timestamp of their own)
and the `src_tx_hash` / `dst_tx_hash` flags — never to move the predicate onto
the message.

The clearest consequence already in the code: `get_min_date_interchain` takes the
`min` of two *separately* filtered floors rather than one, precisely because a
transfer can satisfy `transfers_condition()` while its own message fails
`messages_condition()`. Using the message floor alone would silently truncate the
transfer charts' history.

**Standing assumption — the one place this is knowingly glossed over.** The
interchain catch-up scoping (see
`.memory-bank/research/update-range-anchoring-and-backfill-detection.md`) narrows
the relevant `(bridge_id, chain_id)` pairs by projecting the filter's chain
dimensions onto the **message** route. That is exact for the 4 message chart
families and an **assumption** for the 3 transfer families: it takes a transfer's
token chains to lie inside its message's route-implied slice. Accepted
deliberately, on the understanding that it may stop holding as new bridge types
land.

**How to check whether it still holds** (expect `0`):

```sql
SELECT count(*) FROM crosschain_transfers t
JOIN crosschain_messages m
  ON t.message_id = m.id AND t.bridge_id = m.bridge_id
WHERE t.token_src_chain_id NOT IN (m.src_chain_id, m.dst_chain_id)
   OR t.token_dst_chain_id NOT IN (m.src_chain_id, m.dst_chain_id);
```

**If it stops holding:** scope the 3 transfer families' relevant pairs at
**bridge** level instead of chain level. Bridge narrowing needs no assumption — a
row's `bridge_id` is always its creating pair's bridge — so it is exact at the
cost of being coarser (a catching-up chain on an admitted bridge then holds the
verdict even when its rows cannot enter the slice).

---

## Interchain *Line* Charts Inherit A Blockscout Indexing Requirement, But It Is Neutralised By Seeding

**Symptom:** `/api/v1/update-status` fields are **not** interchangeable in
`Mode::Interchain`, contrary to the natural assumption that "interchain has no
indexing axis, so every subset covers every chart". `independent_status` and
`zetachain_cctx_dependent_status` cover only the 7 counters; the 32 line charts
land in the blocks-dependent subsets. The fields can therefore differ
transiently.

**Root cause:** The 7 interchain counters each override
`ChartProperties::indexing_status_requirement()` to
`IndexingStatus::LEAST_RESTRICTIVE`. **No** interchain line chart overrides it, so
all 8 families × 4 resolutions inherit the default from
`stats/src/charts/chart.rs:199` — `blockscout: BlockscoutIndexingStatus::BlocksIndexed`,
whose comment reads "most of the charts need indexed blocks". Interchain charts
read `crosschain_messages` / `crosschain_transfers` and have no relationship to
Blockscout blocks at all, so the inherited requirement is meaningless here — it
just is not harmful.

**Why it is not harmful — the part that is easy to get backwards.** It would be
tempting to conclude that wiring an `IndexingStatusAggregator` into interchain
mode would make those 32 charts block forever on a Blockscout status that never
arrives. **It would not.** `blockscout_waiter::init` seeds the axis by what is
*enabled*, not by what has been observed:

```rust
match (blocks_ratio.enabled, internal_transactions_ratio.enabled) {
    (true, _)      => BlockscoutIndexingStatus::NoneIndexed,
    (false, true)  => BlockscoutIndexingStatus::BlocksIndexed,
    (false, false) => BlockscoutIndexingStatus::InternalTransactionsIndexed,
}
```

`apply_interchain_mode_settings` disables both, so the axis seeds at
`InternalTransactionsIndexed`, which is `BlockscoutIndexingStatus::MAX`. And
`is_requirement_satisfied` is `self >= requirement` over an ordered enum
(`NoneIndexed < BlocksIndexed < InternalTransactionsIndexed`), so `BlocksIndexed`
is satisfied on the first poll. `init`'s own comment states the intent: *"enable
immediately if the checks are disabled."*

**The real trap is the opposite one — an axis enabled with no source.**
`check_zetachain_status` warns and returns when `zetachain_cctx_db` is `None`,
leaving the axis at its `CatchingUp` seed forever, so every dependent group blocks
indefinitely. `IndexingStatusAggregator::run` has a second instance of the same
shape: it early-returns on
`!blockscout_checks_enabled() && !user_ops_checks_enabled()` without consulting
`zetachain_checks_enabled()`. Any *new* axis must therefore either fail at startup
when enabled without its source — the way `init_blockscout_api_client`'s
`(false, None)` arm `bail!`s — or disable itself with a warning. Never seed
"not ready" and hope the source appears.

**Fix:** If interchain line charts ever need to participate in a wait, give them
an explicit `indexing_status_requirement()` override rather than relying on the
default; the inherited value is accidental, not intentional. Do not relax the
default in `chart.rs` — it is correct for the Blockscout-mode majority.

**Resolved.** The interchain historical-backfill work
(`.memory-bank/research/update-range-anchoring-and-backfill-detection.md`) added
a 4th `IndexingStatus` axis (`interchain: InterchainIndexingStatus`) and gave all
15 interchain chart families — the 7 counters and, now, all 8 line families —
an explicit `indexing_status_requirement()` declaring it. The "no line chart
overrides it" half of this entry's symptom no longer applies; the seeding
mechanics above (seed-satisfied-when-disabled, fail-at-startup-when-enabled-
without-a-source) are exactly what the new axis follows, and are why they are
kept here rather than duplicated.

---

## `env-collector`'s Per-Field Default Column Can Disagree With The Real Default

**Symptom:** `just generate-envs`/`just check-envs` render a settings field's
"Default value" column as something that does not match what
`Settings::default()` (or the containing struct's own hand-written `Default`)
actually produces for that field — and a test asserting the real default (e.g.
`StartConditionSettings::default()`) passes anyway, so the code is correct and
only the generated table is wrong.

**Root cause:** `env-collector`'s `default_of_var` (`libs/env-collector/src/lib.rs`)
computes "what does field X default to" by serializing the whole settings
struct to JSON, **removing only X's own leaf key**, and re-deserializing —
relying on serde's `#[serde(default)]` to refill the missing leaf. That works
when the refill source is the *outer* struct's own `Default` impl, but when the
leaf's immediate parent type carries its **own** `#[serde(default)]` (e.g.
`ToggleableThreshold`, whose `Default` is `Self::enabled(0.98)` — `enabled:
true`), removing just the leaf triggers *that* type's container-level default
for the leaf, not the outer struct's override of the whole sub-object. A
`StartConditionSettings` field written out as
`ToggleableThreshold::disabled().set_threshold(0.98)` (`enabled: false`) then
gets its `…__ENABLED` row rendered as `true` — `ToggleableThreshold::default()`'s
own `enabled`, not the actual field value.

**This is not new to this task** — `STATS__CONDITIONAL_START__BLOCKS_RATIO__ENABLED`
has always rendered `true` in `README.md`, and that happens to be correct only
because `blocks_ratio`'s real default equals `ToggleableThreshold::default()`
unmodified. The interchain catch-up gate
(`STATS__CONDITIONAL_START__INTERCHAIN_CATCHUP_MIN_PROGRESS__ENABLED`, real
default `false`) is the first `ToggleableThreshold` field whose override
diverges from `ToggleableThreshold::default()`, which is what makes the quirk
visible as a wrong answer for the first time.

**What this does not affect:** `just check-envs` still passes, because it
compares the checked-in table against a freshly recomputed one using the same
(quirky) function — there is no independent ground truth being checked against.
The quirk only misleads a human reading the generated default column.

**Fix:** none applied — `libs/env-collector` is a shared library used by other
services' doc generation, so this task did not modify it. Do not "fix" the
rendered `true` by hand-editing the default column; `generate-envs` would
recompute the same value and `check-envs` would flag the hand-edit as drift on
the next run. If this ever needs correcting, it belongs in
`env-collector::default_of_var`, scoped and tested against `blocks_ratio` too.
