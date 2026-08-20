# Interchain Mode: Data Flow and Read Filtering

> Labelling convention used throughout: **[F]** = fact verified in the current
> source; **[I]** = inference / reading of the code that is not itself written
> down anywhere; **[Q]** = open question that needs a human decision or a
> runtime experiment. All paths are repo-relative to `blockscout-rs/`.

> **Status — read this first.** Sections 1, 2, 4, 5, 6 and 8-10 describe machinery
> that is still current. **Section 3 and the table in section 6.7 describe the
> mechanism as it was *before* configurable read filtering landed**, and are kept
> as the baseline the change was designed against, not as a description of the
> code today. What replaced them:
>
> | was | is now |
> |---|---|
> | one scalar `STATS__INTERCHAIN_PRIMARY_ID` | six `STATS__INTERCHAIN_FILTER__*` variables mirroring the indexer read API, with `INTERCHAIN_PRIMARY_ID` honoured as a deprecated alias for `HOME_CHAIN_ID` |
> | three hand-rolled predicate shapes per chart | one `ChainBridgeFilter` from the shared `interchain-indexer-filters` crate, applied by every chart and counter |
> | 4 of 15 chart families unfiltered | none — coverage is asserted by a test |
> | transfers filtered through the joined message's route | transfers filtered on their own `token_src_chain_id` / `token_dst_chain_id` / `bridge_id` |
> | `ON t.message_id = m.id` | composite `(message_id, bridge_id)` join |
> | no observability horizon | horizon resolved per update cycle from `bridges` + `bridge_contracts`, on by default |
> | changing the filter silently mixed two regimes in one history | filter fingerprint stored per point; a change clears the chart and recomputes |
>
> Landed in `c65e886b`, `020d210d`, `d3ae7378` (docs in `69a52f22`), on top of the
> test-harness change in `ae228842`. Section 6.8 records what still diverges from
> the indexer and what a local end-to-end run measured.

## Scope

Covered:

- what `Mode::Interchain` is, how it is selected and wired end to end;
- the complete inventory of interchain charts/counters and their update
  pipeline (interchain-indexer DB → stats DB → read service);
- **the current filtering mechanism in depth**: `interchain_primary_id`, the
  exact SQL predicate every chart builds, and which charts ignore it;
- the indexer-side schema stats actually reads, and what it does not read;
- timespan/range machinery in interchain mode and its interaction with filtering;
- how the interchain-indexer's own read-time API filtering works, and exactly
  where stats diverges from it (the parity gap);
- how interchain charts are tested and what the fixtures encode.

Intentionally out of scope:

- the *intended future* design of stats-side filtering (separate task analysis);
- non-interchain modes except where they define shared machinery;
- the indexer's write-time (indexing) filtering, except as context.

## Short Answer

`Mode::Interchain` points the stats service at an `interchain-indexer` Postgres
schema and enables a small, self-contained chart set: **7 counters and 8 line
charts** (each line chart in day/week/month/year resolutions). All of them read
only two indexer tables, `crosschain_messages` and `crosschain_transfers`.

Filtering *as this note found it* was a **single scalar knob**,
`Settings.interchain_primary_id: Option<u64>` (`STATS__INTERCHAIN_PRIMARY_ID`)
— see the status header for what replaced it. It is an **update-time**
(write-into-stats-DB) concern only: it is threaded into `UpdateContext` and
spliced into the remote SQL each chart builds; it is hard-coded to `None` on the
read path, so serving stored chart data never sees it. When set, it produces one
of exactly three predicate shapes — `src_chain_id = $1`, `dst_chain_id = $1`, or
`(m.src_chain_id = $1 OR m.dst_chain_id = $1)` — and **4 of the 15 chart
families ignore it entirely**.

The indexer's read API, by contrast, owns a six-dimensional
`ChainBridgeFilter` (focal home/counterparty, directional src/dst, bridge, and
the `only_indexed_by_bridge` observability horizon), applies it per request, and
filters **transfers on the transfer's own columns**. Stats filters transfers by
joining to the parent message. That is the core of the parity gap.

## Why This Matters

- Any stats number shown next to an interchain-indexer API number can disagree,
  and the disagreement is structural rather than a bug in one query.
- `interchain_primary_id` was a deploy-time constant baked into stored
  aggregates; changing it did not invalidate anything already written
  (`stats-server/src/settings.rs:100` carried the `TODO` admitting this), so a
  changed value silently yielded a chart whose history mixed two filter regimes.
  **Resolved** by the filter fingerprint in `d3ae7378`: a config change now
  clears the affected charts and recomputes them.
- The stats test harness defined its **own** approximation of the indexer schema,
  so the columns and cardinalities that the real parity work depends on
  (`bridge_id`, `token_src_chain_id`, nullable `dst_chain_id`) existed in no test.
  **Resolved** by `ae228842`: the interchain test schema is now built by the
  indexer's own migrator.

## Source-of-Truth Files

### stats (this repo)

| Path | Role |
|---|---|
| `stats/stats/src/mode.rs` | `Mode` enum; `Interchain` variant (L18-19) |
| `stats/stats-server/src/settings.rs` | `Settings.mode` (L63), `interchain_primary_id` (L102, doc block L98-102 with the `TODO` at L100), default `None` (L217), `apply_interchain_mode_settings` (L423-435) |
| `stats/stats-server/src/server.rs` | mode dispatch (L65), `interchain_primary_id` → `UpdateServiceConfig` (L99), `ReadService` gets `settings.mode` only (L124) |
| `stats/stats-server/src/update_service.rs` | config field (L35), struct field (L44), constructor (L82), `UpdateParameters` build (L435) |
| `stats/stats/src/data_source/types.rs` | `UpdateParameters.interchain_primary_id` (L28), `query_parameters` forces `None` (L62), `UpdateContext.interchain_primary_id` (L128), propagation (L148) |
| `stats/stats/src/charts/counters/interchain/` | the 7 counters |
| `stats/stats/src/charts/lines/interchain/` | the 8 line-chart families |
| `stats/stats/src/update_groups_interchain.rs` | 7 singleton counter groups + 6 `construct_update_group!` groups |
| `stats/stats/src/charts/db_interaction/read/interchain.rs` | `get_min_date_interchain` (L13), `get_min_block_interchain` (L35) |
| `stats/stats/src/charts/db_interaction/read/mod.rs` | `QueryFullIndexerTimestampRange` (L38), `get_min_date` dispatch (L54, Interchain arm L59) |
| `stats/stats/src/data_source/kinds/local_db/mod.rs` | `min_indexer_block` dispatch (L189) |
| `stats/stats/src/utils.rs` | `produce_filter_and_values` (L42), `sql_with_range_filter_opt!` (L154) |
| `stats/stats-server/src/read_service.rs` | `main_page_interchain_charts` (L316), `get_main_page_interchain_stats` (L865) |
| `stats/config/interchain/{charts,layout,update_groups}.json` | which charts are enabled/ordered/scheduled |
| `stats/stats/src/tests/init_db.rs` | local hand-rolled interchain migrator (L11-83) |
| `stats/stats/src/tests/mock_interchain.rs` | the fixture rows |
| `stats/stats/src/tests/simple_test.rs` | `simple_test_chart_interchain` (L138), `simple_test_counter_interchain` (L480) |
| `stats/README.md:79` | generated env-doc row for `STATS__INTERCHAIN_PRIMARY_ID` |

### interchain-indexer (sibling service; read-only reference)

| Path | Role |
|---|---|
| `interchain-indexer/interchain-indexer-logic/src/filters.rs` | `ChainBridgeFilter` (L11-31); `messages_condition` (L44), `transfers_condition` (L113) |
| `interchain-indexer/interchain-indexer-logic/src/stats/indexed_chains.rs` | `IndexedChains` (L23), `may_observe` (L94), `configured_pairs` (L161), `message_has_unindexed` (L235), `transfer_has_unindexed` (L240) |
| `interchain-indexer/interchain-indexer-server/src/services/utils.rs` | `build_chain_bridge_filter` (L94) |
| `interchain-indexer/interchain-indexer-server/src/services/stats.rs` | `get_common_statistics` (L45) and `get_daily_statistics` (L80) handlers; `include_unindexed_chains.unwrap_or(false)` (L62, L97) |
| `interchain-indexer/interchain-indexer-logic/src/database.rs` | `get_total_counters` (L3340), `get_daily_counters` (L3376), `upsert_bridges` (L929), `upsert_bridge_contracts` (L1914) |
| `interchain-indexer/interchain-indexer-server/src/server.rs` | `IndexedChains::from_bridges` from the **config file** (L296-309) |
| `interchain-indexer/interchain-indexer-migration/src/migrations_up/m20251030_000001_initial_up.sql` | canonical DDL (`crosschain_messages` L66, `crosschain_transfers` L132) |
| `interchain-indexer/interchain-indexer-entity/src/codegen/{crosschain_messages,crosschain_transfers}.rs` | generated entity models |
| `interchain-indexer/.memory-bank/adr/004-stats-observability-horizon-and-asset-union-find.md` | Decisions 1 and 5 |
| `interchain-indexer/.memory-bank/research/stats-subsystem.md` | endpoint-by-endpoint calculation rules |

---

## 1. What `Mode::Interchain` Is, and How It Is Selected and Wired

**[F]** `Mode` is a four-variant enum shared by the library and the server
(`stats/stats/src/mode.rs`):

```rust
pub enum Mode {
    /// Single blockscout instance
    Blockscout,
    /// Multichain aggregator
    MultichainAggregator,
    /// Zetachain instance
    Zetachain,
    /// Interchain indexer (a.k.a. Universal Bridge Indexer)
    Interchain,
}
```

**[F]** Selection is a single setting, `STATS__MODE=interchain` (serde
`rename_all = "snake_case"`). Modes are documented as mutually exclusive by
design (`stats-server/src/settings.rs`, doc comment on `Settings.mode`).

**[F]** Startup wiring, in order (`stats-server/src/server.rs`):

1. `read_charts_config` / `read_layout_config` / `read_update_groups_config`
   load whatever paths the `charts_config` / `layout_config` /
   `update_groups_config` settings point at. **The mode does not select the
   config directory** — the defaults are the `blockscout_instance` ones
   (`settings.rs` `Default` impl), so an interchain deployment must point the
   three config settings at `config/interchain/` explicitly. `just run-interchain`
   (`stats/justfile:98-109`) does exactly that.
2. `match settings.mode { … Mode::Interchain => apply_interchain_mode_settings(&mut settings) … }`
   (`server.rs:65`). That function (`settings.rs:423`) only *disables* things:
   `blockscout_api_url = None`, `ignore_blockscout_api_absence = true`, and it
   turns off the `blocks_ratio`, `internal_transactions_ratio` and
   `user_ops_past_indexing_finished` conditional-start gates. It touches no
   chart config and no filter.
3. `connect_to_main_indexer_db` yields the single indexer connection; the second
   (CCTX) DB is `None` in every mode but `Zetachain`.
4. `UpdateService` is built with `mode` and `interchain_primary_id`
   (`server.rs:99`), `ReadService` with `mode` only (`server.rs:124`) — **the
   read service is never given the filter**.

**[F]** `RuntimeSetup::all_update_groups` (`stats-server/src/runtime_setup.rs:449`)
registers *all* modes' groups unconditionally, including the 13 interchain
groups (L519-532). Which of them actually run is decided by the loaded
`charts.json` / `update_groups.json`, not by `Mode`.

**[F]** Mode branching inside the library is narrow — the only `Mode::Interchain`
match arms outside tests are:

- `stats/src/charts/db_interaction/read/mod.rs:59` (`get_min_date`)
- `stats/src/data_source/kinds/local_db/mod.rs:189` (`min_indexer_block`)

Everything else that is interchain-specific is selected by *config*, not by a
runtime `match`.

**[F]** `IndexerMigrations::query_from_db` (`data_source/types.rs:167`) returns
`IndexerMigrations::empty()` for every mode except `Blockscout`/`Zetachain`, so
`cx.indexer_applied_migrations.denormalization` is always `false` in interchain
mode. (Already recorded in `.memory-bank/gotchas.md`.)

---

## 2. Charts and Counters in Interchain Mode, and Their Update Pipeline

### 2.1 Inventory

**[F]** 7 counters (`stats/src/charts/counters/interchain/mod.rs`, all enabled and
ordered in `config/interchain/{charts,layout}.json`):

| public id | Rust type | filters on `interchain_primary_id`? | column(s) |
|---|---|---|---|
| `total_interchain_messages` | `TotalInterchainMessages` | **no** | — (`COUNT(*)` over `crosschain_messages`) |
| `total_interchain_messages_sent` | `TotalInterchainMessagesSent` | yes | `crosschain_messages.src_chain_id` |
| `total_interchain_messages_received` | `TotalInterchainMessagesReceived` | yes | `crosschain_messages.dst_chain_id` |
| `total_interchain_transfers` | `TotalInterchainTransfers` | **no** | — (`COUNT(*)` over `crosschain_transfers`) |
| `total_interchain_transfers_sent` | `TotalInterchainTransfersSent` | yes | **`m.src_chain_id`** (joined message) |
| `total_interchain_transfers_received` | `TotalInterchainTransfersReceived` | yes | **`m.dst_chain_id`** (joined message) |
| `total_interchain_transfer_users` | `TotalInterchainTransferUsers` | yes | **`m.src_chain_id` OR `m.dst_chain_id`** (joined message) |

**[F]** 8 line-chart families (`stats/src/charts/lines/interchain/mod.rs`), each
in 4 resolutions (base/`Weekly`/`Monthly`/`Yearly`) → 32 registered line charts:

| public id | Rust type | filters? | column(s) |
|---|---|---|---|
| `new_messages_interchain` | `NewMessagesInterchain` | **no** | — (`WHERE true`) |
| `new_messages_sent_interchain` | `NewMessagesSentInterchain` | yes | `src_chain_id` |
| `new_messages_received_interchain` | `NewMessagesReceivedInterchain` | yes | `dst_chain_id` |
| `messages_growth_sent_interchain` | `MessagesGrowthSentInterchain` | yes, **indirectly** | inherits from `NewMessagesSentInterchainInt` |
| `messages_growth_received_interchain` | `MessagesGrowthReceivedInterchain` | yes, **indirectly** | inherits from `NewMessagesReceivedInterchainInt` |
| `new_transfers_interchain` | `NewTransfersInterchain` | **no** | — (`WHERE true`, still joins to messages) |
| `new_transfers_sent_interchain` | `NewTransfersSentInterchain` | yes | **`m.src_chain_id`** |
| `new_transfers_received_interchain` | `NewTransfersReceivedInterchain` | yes | **`m.dst_chain_id`** |

So of the **15 chart families, 4 ignore the filter entirely**:
`total_interchain_messages`, `total_interchain_transfers`,
`new_messages_interchain`, `new_transfers_interchain`.

**[F]** Update groups (`stats/src/update_groups_interchain.rs`): the 7 counters
are `singleton_groups!`; the line charts form 6 groups — the two `*_growth_*`
families are **members of the corresponding `New*SentInterchainGroup` /
`New*ReceivedInterchainGroup`**, not their own groups. That is 13 groups, matching
the 13 schedules in `config/interchain/update_groups.json` (counters and
message/transfer line charts every 2 hours; `total_interchain_transfer_users_group`
once daily at 00:30).

### 2.2 Pipeline

**[F]** Counters: `StatementFromUpdateTime` → `RemoteDatabaseSource<PullOneNowValue<…>>`
→ `MapToString` → `DirectPointLocalDbChartSource`. `impl_db_choice!(…, UsePrimaryDB)`
means the statement runs against `cx.indexer_db`
(`data_source/kinds/remote_db/db_choice.rs:13-17`). `PullOneNowValue`
(`remote_db/query/one.rs:80-91`) calls `S::get_statement_with_context(cx)` and
stamps the result with `Resolution::from_date(cx.time.date_naive())`.

**[F]** Line charts: `StatementFromRange` → `RemoteDatabaseSource<PullAllWithAndSort<S,
NaiveDate, String, QueryFullIndexerTimestampRange>>` →
`DirectVecLocalDbChartSource<…, Batch30Days, Properties>`; lower resolutions are
`SumLowerResolution` over the base chart (`Batch30Weeks` / `Batch36Months` /
`Batch30Years`). The two growth charts are
`DailyCumulativeLocalDbChartSource<New…Int, Properties>` with
`LastValueLowerResolution` for weekly/monthly/yearly.

**[F]** Read path: `ReadService` serves counters and line charts out of the local
stats DB. `UpdateParameters::query_parameters` (`data_source/types.rs:50-75`)
constructs read-time parameters with `interchain_primary_id: None` and the
comment `// only used when updating the DB`. There is **no interchain-specific
read endpoint other than** `get_main_page_interchain_stats`
(`read_service.rs:865`), which serves three already-stored counters
(`totalInterchainMessages`, `…Sent`, `…Received`) and optionally merges a linked
secondary stats instance.

**[I]** Consequence: filtering in stats is a *materialisation* decision, not a
query decision. One stats deployment can serve exactly one filter regime, and
that regime is frozen into the rows of the stats DB.

---

## 3. The Pre-Change Filtering Mechanism, In Depth

> **Superseded** by `c65e886b` / `020d210d` / `d3ae7378` — see the status header.
> Kept because the parity work was specified against exactly these shapes, and
> because the deprecated `STATS__INTERCHAIN_PRIMARY_ID` still resolves to the
> `home_chain_id` arm described here.

### 3.1 Where it enters and how it threads through

**[F]** `stats-server/src/settings.rs:98-102`:

```rust
    /// Set the primary chain_id for Interchain mode
    /// If the primary chain set, send/receive counters and charts will be built around it
    /// TODO: recalculate statistics data when interchain_primary_id has been changed
    ///       most likely it's need to implement in conjunction with 3D charts
    pub interchain_primary_id: Option<u64>,
```

Chain of custody, every hop verified:

```
STATS__INTERCHAIN_PRIMARY_ID  (env; default null — settings.rs:217)
  → Settings.interchain_primary_id                 settings.rs:102
  → UpdateServiceConfig.interchain_primary_id      server.rs:99  /  update_service.rs:35
  → UpdateService.interchain_primary_id            update_service.rs:44, set at :82
  → UpdateParameters.interchain_primary_id         update_service.rs:435  /  types.rs:28
  → UpdateContext.interchain_primary_id            types.rs:128 (copied at :148)
  → each chart's get_statement_with_context(cx)    per-chart match on cx.interchain_primary_id
```

**[F]** There is exactly one other producer of `UpdateParameters`:
`query_parameters` (`types.rs:50`), which hard-codes `interchain_primary_id: None`
(L62). Tests add `default_test_parameters` (L90) which also sets `None`.

**[F]** The type is `Option<u64>`; every consumer converts with
`sea_orm::Value::BigInt(Some(primary_id as i64))`. **[I]** A value above
`i64::MAX` would wrap silently; in practice chain ids are far below that, and the
indexer stores chain ids as `BIGINT`, so this is theoretical.

### 3.2 The exact predicates

There are only three shapes. All are string-spliced into a raw SQL literal with
`$1` as the bound parameter (never interpolated as text), and `interchain_primary_id`
is the only `$1` any of these queries has.

**Shape A — messages, direct column.** `crosschain_messages` is queried directly.

`total_interchain_messages_sent.rs:17-21`:

```sql
                SELECT COUNT(*)::bigint AS value
                FROM crosschain_messages
                WHERE src_chain_id = $1 AND src_tx_hash IS NOT NULL
```

`total_interchain_messages_received.rs:17-21` is the mirror image
(`dst_chain_id = $1 AND dst_tx_hash IS NOT NULL`). The `None` arms drop only the
chain term, keeping the `*_tx_hash IS NOT NULL` term.

The two line-chart equivalents build the term as a `String` and hand it to
`sql_with_range_filter_opt!`. `new_messages_sent_interchain.rs:36-57`:

```rust
        let (chain_condition, values) = match cx.interchain_primary_id {
            Some(primary_id) => (
                " AND src_chain_id = $1".into(),
                vec![sea_orm::Value::BigInt(Some(primary_id as i64))],
            ),
            None => (String::new(), vec![]),
        };
```

into (same file, L45-52)

```sql
                SELECT
                    init_timestamp::date AS date,
                    COUNT(*)::TEXT AS value
                FROM crosschain_messages
                WHERE src_tx_hash IS NOT NULL {chain_condition} {filter}
                GROUP BY init_timestamp::date
```

`new_messages_received_interchain.rs:38` uses `" AND dst_chain_id = $1"` with the
same SQL shape (L45-52).

**Shape B — transfers, filtered through the joined message.** The stats
transfer queries all join `crosschain_transfers` to `crosschain_messages` and
put the chain predicate on the **message**.

`total_interchain_transfers_sent.rs:16-21`:

```sql
                SELECT COUNT(*)::bigint AS value
                FROM crosschain_transfers t
                INNER JOIN crosschain_messages m ON t.message_id = m.id
                WHERE m.src_tx_hash IS NOT NULL AND m.src_chain_id = $1
```

`total_interchain_transfers_received.rs:16-21` is the `dst` mirror.
`new_transfers_sent_interchain.rs:38` / `new_transfers_received_interchain.rs:38`
build `" AND m.src_chain_id = $1"` / `" AND m.dst_chain_id = $1"` into (both at
L45-53)

```sql
                SELECT
                    m.init_timestamp::date AS date,
                    COUNT(*)::TEXT AS value
                FROM crosschain_transfers t
                INNER JOIN crosschain_messages m ON t.message_id = m.id
                WHERE m.src_tx_hash IS NOT NULL {chain_condition} {filter}
                GROUP BY m.init_timestamp::date
```

**Shape C — the only disjunctive predicate in stats.**
`total_interchain_transfer_users.rs:16-31`
(the `interchain_primary_id` match is at L13-50):

```sql
                SELECT COUNT(*)::bigint AS value
                FROM (
                    SELECT t.sender_address AS addr
                    FROM crosschain_transfers t
                    INNER JOIN crosschain_messages m ON t.message_id = m.id
                    WHERE t.sender_address IS NOT NULL
                      AND (m.src_chain_id = $1 OR m.dst_chain_id = $1)
                    UNION
                    SELECT t.recipient_address AS addr
                    FROM crosschain_transfers t
                    INNER JOIN crosschain_messages m ON t.message_id = m.id
                    WHERE t.recipient_address IS NOT NULL
                      AND (m.src_chain_id = $1 OR m.dst_chain_id = $1)
                ) u
```

**[I]** Shape C is the only place stats expresses anything like the indexer's
focal `(Some(home), None)` case. Note the `None` arm of this counter drops the
join entirely (`FROM crosschain_transfers` with no join) — so the filtered and
unfiltered variants are structurally different queries, not one query with an
extra term.

### 3.3 Charts that ignore the filter

**[F]** Four families take `_cx: &UpdateContext<'_>` and never look at
`interchain_primary_id`:

- `total_interchain_messages.rs:11-22` (SQL L15-18) — `SELECT COUNT(*)::bigint AS value FROM crosschain_messages`
- `total_interchain_transfers.rs:11-22` (SQL L15-18) — `SELECT COUNT(*)::bigint AS value FROM crosschain_transfers`
- `new_messages_interchain.rs:29-50` (SQL L36-44) — `FROM crosschain_messages WHERE true {filter}`
- `new_transfers_interchain.rs:29-51` (SQL L36-45) — `FROM crosschain_transfers t INNER JOIN crosschain_messages m ON t.message_id = m.id WHERE true {filter}`

Each carries a module doc line saying so, e.g. `total_interchain_messages.rs:4`:
`//! Counts all rows in crosschain_messages. Does not use interchain_primary_id.`

**[I]** These four are "total indexed" charts by intent — they describe the
indexer instance, not the primary chain. But nothing in the code or config
prevents them from being served side by side with the filtered ones, and the
interchain main-page endpoint does exactly that: `total_interchain_messages`
(unfiltered) next to `…_sent` / `…_received` (filtered). **[Q]** Should the
"total" charts follow the new filter, or keep describing the whole DB? That is a
product decision, not something the code answers.

> **Settled since.** All of them follow the filter (§11, question 2). Their
> `charts.json` descriptions were *not* updated to match — see
> `gotchas.md`, "Interchain Chart Titles *and Descriptions* … Are Frozen UI
> Strings".

### 3.4 Two structural properties of shape B worth naming

**[F]** The join condition is `ON t.message_id = m.id` — **`bridge_id` is not
part of it**, while the real primary key of `crosschain_messages` is
`(id, bridge_id)` and the real FK from `crosschain_transfers` is the composite
`(message_id, bridge_id) → (id, bridge_id)`
(`m20251030_000001_initial_up.sql:90` and `:151-153`;
`crosschain_transfers.rs:54-61`).

**[I]** With two bridges owning rows with the same `crosschain_messages.id`, one
transfer joins to *both* message rows, so shape-B queries can over-count. `id`
is application-assigned (`auto_increment = false` on both PK columns in the
entity), so cross-bridge id collisions are plausible rather than exotic — the
indexer added an explicit `bridge_id` qualifier and an `Ambiguous` lookup result
to the single-message-details endpoint precisely because unqualified ids can be
ambiguous across bridges (PR #1708). **[Q]** Does the current production
interchain-indexer deployment run more than one bridge against the stats DB
stats reads? If yes, the shape-B counters are already inflated today.

**[F]** None of the interchain counters bound their count by update time. The
indexer's `/stats/common` uses `init_timestamp < timestamp`
(`database.rs:3350`); stats' `total_*` counters have no timestamp predicate at
all. **[I]** A row with a future `init_timestamp` therefore counts in stats and
not in `/stats/common`.

---

## 4. The Indexer-Side Schema Stats Reads

Canonical DDL: `interchain-indexer-migration/src/migrations_up/m20251030_000001_initial_up.sql`
(with `ALTER`s in `m20260508_082944_add_amb_indexer_up.sql` and indexes added in
`m20260720_120000_add_read_filters_and_bridge_stats_up.sql`).

### `crosschain_messages` (DDL L66-91)

| column | type / nullability | used by stats? |
|---|---|---|
| `id` | `BIGINT NOT NULL`, part of PK | yes — join key only |
| `bridge_id` | `INTEGER NOT NULL REFERENCES bridges(id)`, part of PK | **no** |
| `status` | `message_status NOT NULL DEFAULT 'initiated'` | **no** |
| `init_timestamp` | `TIMESTAMP NOT NULL DEFAULT now()` | yes — the sole time axis |
| `src_chain_id` | `BIGINT NOT NULL REFERENCES chains(id)` | yes — filter + `*_sent` |
| `dst_chain_id` | **`BIGINT NULL`** `REFERENCES chains(id)` | yes — filter + `*_received` |
| `src_tx_hash` | `BYTEA` (nullable) | yes — `IS NOT NULL` = "sent" |
| `dst_tx_hash` | `BYTEA` (nullable) | yes — `IS NOT NULL` = "received" |
| `last_update_timestamp`, `native_id`, `sender_address`, `recipient_address`, `payload`, `created_at`, `updated_at`, `stats_processed` | various | **no** |

`PRIMARY KEY (id, bridge_id)`.

### `crosschain_transfers` (DDL L132-156)

| column | type / nullability | used by stats? |
|---|---|---|
| `id` | `BIGSERIAL PRIMARY KEY` | no (only `COUNT(*)`) |
| `message_id` | `BIGINT NOT NULL` | yes — join |
| `bridge_id` | `INTEGER NOT NULL` | **no** |
| `token_src_chain_id` | `BIGINT NOT NULL REFERENCES chains(id)` | **no** ← the parity gap |
| `token_dst_chain_id` | `BIGINT NOT NULL REFERENCES chains(id)` | **no** ← the parity gap |
| `sender_address` | `BYTEA` (nullable) | yes — `total_interchain_transfer_users` |
| `recipient_address` | `BYTEA` (nullable) | yes — `total_interchain_transfer_users` |
| `index`, `type`, `src_amount`, `dst_amount`, `token_src_address`, `token_dst_address`, `token_ids`, `stats_processed`, `stats_asset_id`, `created_at`, `updated_at` | various | **no** |

Composite FK `(message_id, bridge_id) → crosschain_messages(id, bridge_id) ON DELETE CASCADE`,
plus `UNIQUE (message_id, bridge_id, index)`.

**[F]** `src_amount` / `dst_amount` / `token_src_address` / `token_dst_address`
were made nullable by `m20260508_082944_add_amb_indexer_up.sql:63-67` (ADR-003 —
AMB Omnibridge transfers are reconstructed from events, so the unseen side is
genuinely unknown). `token_src_chain_id` / `token_dst_chain_id` were **not**
touched and remain `NOT NULL`.

### Tables stats does not read at all

**[F]** `bridges`, `bridge_contracts`, `chains`, `pending_messages`,
`amb_messages_confirmations`, `amb_message_anomalies`, `bridge_txs`, `tokens`,
`indexer_checkpoints`, `indexer_failures`, `avalanche_icm_blockchain_ids`, and
the whole projected-stats family (`stats_messages`, `stats_messages_days`,
`stats_asset_edges`, `stats_assets`, `stats_asset_tokens`, `stats_chains`).
Verified by grepping every table name across `stats/`: only
`crosschain_messages` and `crosschain_transfers` appear.

**[I]** Notably, stats does **not** consume the indexer's own projected
aggregates. It re-aggregates the canonical tables from scratch on its own
schedule. So the indexer's projection eligibility rules (finality, deferral,
observability horizon) do not constrain stats at all.

### Read-filter indexes that exist for the indexer's benefit

**[F]** `m20260720_120000_…_up.sql:6-12` adds
`crosschain_messages_bridge_ts_idx ON crosschain_messages (bridge_id, init_timestamp, id)`
and `crosschain_transfers_bridge_idx ON crosschain_transfers (bridge_id)`; the
initial migration already provides
`crosschain_messages_pagination_idx (init_timestamp, id, bridge_id)`,
`crosschain_transfers_token_src_idx (token_src_chain_id, token_src_address)` and
`crosschain_transfers_token_dst_idx (token_dst_chain_id, token_dst_address)`.

**[F]** Stats' current shape-A predicates **are** index-supported. The initial
migration ships two chain-leading indexes, commented "Statistics queries: filter
by chain IDs with timestamp range"
(`interchain-indexer-migration/src/migrations_up/m20251030_000001_initial_up.sql:111-116`):

```sql
CREATE INDEX crosschain_messages_src_chain_ts_idx
    ON crosschain_messages (src_chain_id, init_timestamp);

CREATE INDEX crosschain_messages_dst_chain_ts_idx
    ON crosschain_messages (dst_chain_id, init_timestamp)
    WHERE dst_chain_id IS NOT NULL;
```

**[I]** What is *not* covered is a `(src_chain_id, dst_chain_id)` pair, so the
focal `(Some(home), Some(counterparties))` shape plans as a BitmapOr of the two
indexes above plus a filter step. **[Q]** Worth an `EXPLAIN` on production-sized
data before adding more chain predicates. `stats` cannot add indexes to a schema
it does not own — any need found is a change request against
`interchain-indexer`.

---

## 5. Timespan / Range Machinery in Interchain Mode

**[F]** `get_min_date_interchain` (`read/interchain.rs:13-31`) is the whole
lower-bound story:

```sql
        SELECT MIN(init_timestamp::timestamp) AS min_timestamp
        FROM crosschain_messages
```

No `WHERE` clause — **it is not filtered by `interchain_primary_id`**, not
filtered by bridge, and not filtered by `src_tx_hash`/`dst_tx_hash`. If the
table is empty it falls back to `Utc::now().naive_utc()`.

**[F]** `get_min_block_interchain` (`read/interchain.rs:35-37`) returns
`i64::MAX` unconditionally, with the doc comment
`/// InterchainIndexer does not have block information; we return i64::MAX so that
last_accurate_point logic does not trigger reupdates based on block.` The value
is passed as `min_indexer_block` (`local_db/mod.rs:189`) and persisted as
`chart_data.min_blockscout_block`; `last_accurate_point`
(`db_interaction/read/local_db.rs:430-460`) only compares recorded vs observed,
so a constant means the block-based invalidation path can never fire in
interchain mode.

**[F]** Two consumers of the min date:

1. `QueryFullIndexerTimestampRange` (`read/mod.rs:38-52`) — returns
   `get_min_date(...)..cx.time`, used as the fallback "full range" for every
   interchain line chart's `PullAllWithAndSort`.
2. `BatchUpdate::update_values`
   (`local_db/parameters/update/batching/mod.rs:60-81`) — when there is no
   `last_accurate_point`, the batch start is
   `ChartProps::Resolution::from_date(get_min_date(...).date())`. Batch sizes come
   from `Batch30Days` / `Batch30Weeks` / `Batch36Months` / `Batch30Years`.

**[F]** The range term itself is produced by `produce_filter_and_values`
(`utils.rs:42-61`) and appended **after** the chart's own values:

```rust
            format!(
                " AND
                {filter_by} < ${arg_n_2} AND
                {filter_by} >= ${arg_n_1}"
            ),
```

with `filter_arg_number_start = values.len() + 1`
(`sql_with_range_filter_opt!`, `utils.rs:194`). So when `interchain_primary_id`
is `Some`, the chain term occupies `$1` and the range occupies `$2`/`$3`; when
`None`, the range occupies `$1`/`$2`. **[I]** This ordering is why the chain term
must be `$1` and why the two arms of every `match` must agree on the number of
bound values — an easy thing to break when adding a second filter dimension.

**[F]** `filter_by` per chart: `"init_timestamp::timestamp"` for message charts,
`"m.init_timestamp::timestamp"` for transfer charts. Transfer charts therefore
inherit the **message's** `init_timestamp` as their time axis; `crosschain_transfers`
has no timestamp column of its own.

**[F]** `new_messages_received_interchain` groups by `init_timestamp::date` — the
*message initiation* date, not any destination-side timestamp. Same for
`new_transfers_received_interchain` (`m.init_timestamp::date`).

### Interaction between filtering and range machinery

**[I]** The unfiltered min date means a deployment with
`interchain_primary_id = Some(n)` still starts every batched backfill at the
earliest message in the whole table, even if chain `n`'s first message is years
later. Result: correct values, wasted batches (the first N batches return no
rows). If a future design adds `only_indexed_by_bridge`-style narrowing, this
gets worse, not better.

> **Resolved.** `get_min_date_interchain` now takes the resolved filter and
> `min`s the message-filtered and transfer-filtered floors, so a filtered
> deployment starts its backfill at the first date inside its own slice.

**[I]** Because `min_indexer_block` is a constant and the filter is not part of
any stored fingerprint, changing `STATS__INTERCHAIN_PRIMARY_ID` **cannot**
trigger a re-update. Existing rows keep the old regime; only newly computed
timespans use the new one. The `TODO` at `settings.rs:100` is the only
acknowledgement of this.

> **Resolved.** `min_indexer_block` is no longer a constant in `Interchain`
> mode: `get_min_block_interchain` returns
> `filters::interchain::filter_fingerprint`, which is stamped onto every
> `chart_data` row and compared by `last_accurate_point` on every update. A
> mismatch additionally triggers `clear_all_chart_data` before the recompute,
> because `insert_data_many` has no delete and a *narrowed* filter would
> otherwise leave stale rows on days that now produce none. The fingerprint
> deliberately excludes the observability horizon's resolved pairs (only the
> "is the horizon enabled" boolean is hashed), so an upstream bridge or
> bridge-contract change is still **not** detected — that gap is real and
> unmitigated. See `.memory-bank/gotchas.md`.

---

## 6. The Indexer's Read-Time API Filtering, and the Parity Gap

### 6.1 `ChainBridgeFilter` — six dimensions

**[F]** `interchain-indexer-logic/src/filters.rs:11-31`:

```rust
pub struct ChainBridgeFilter {
    pub home_chain_id: Option<i64>,
    pub counterparty_chain_ids: Option<Vec<i64>>,
    pub src_chain_ids: Option<Vec<i64>>,
    pub dst_chain_ids: Option<Vec<i64>>,
    pub bridge_ids: Option<Vec<i32>>,
    …
    pub only_indexed_by_bridge: Option<Vec<(i32, Vec<i64>)>>,
}
```

Documented invariant: the four `Vec` fields are `Some` only when non-empty.

### 6.2 The focal truth table

**[F]** `filters.rs:49-57` (messages; `transfers_condition` at L127-145 is
identical modulo columns):

| `(home_chain_id, counterparty_chain_ids)` | predicate | combinator |
|---|---|---|
| `(Some(n), Some(s))` | `(src = n AND dst IN s) OR (dst = n AND src IN s)` | `Condition::any()` of two `all()`s |
| `(Some(n), None)` | `src = n OR dst = n` | `Condition::any()` |
| `(None, Some(s))` | `src IN s AND dst IN s` — **"within-set", not a focal OR** | `Condition::all()` |
| `(None, None)` | no predicate | empty `Condition::all()` |

The `(None, Some(s))` row is the non-obvious one: with no home chain, a
counterparty set means "both endpoints inside the set", which is a *conjunction*,
not a disjunction. **[I]** Anyone porting the truth table who assumes all four
cases are disjunctive gets this row wrong and silently widens the result.

### 6.3 Directional and bridge terms live in the outer AND

**[F]** `filters.rs:62-71`:

```rust
        // Directional predicates refine the focal view; they are appended to the
        // outer AND and must never be inserted inside the focal `OR` above.
        let mut cond = Condition::all().add(chain);
        if let Some(s) = self.src_chain_ids.as_deref() {
            cond = cond.add(src.is_in(s.to_vec()));
        }
        if let Some(d) = self.dst_chain_ids.as_deref() {
            cond = cond.add(dst.is_in(d.to_vec()));
        }
        if let Some(b) = self.bridge_ids.as_deref() {
            cond = cond.add(bridge.is_in(b.to_vec()));
        }
```

Same shape for transfers (L150-159). The `only_indexed_by_bridge` disjunction is
also appended to the **outer** AND as one nested `OR` (L72-76 / L160-166), with an
in-code warning that splicing it into the focal `OR` would admit rows from
unselected bridges.

### 6.4 Messages vs transfers: which columns

**[F]** `messages_condition` (columns bound at L45-47) uses
`crosschain_messages::Column::{SrcChainId, DstChainId, BridgeId}`.

**[F]** `transfers_condition` (columns bound at L114-125) uses
`crosschain_transfers::Column::{TokenSrcChainId, TokenDstChainId, BridgeId}` —
the **transfer's own** columns, explicitly qualified with
`crosschain_transfers::Entity`.

**[F]** The indexer *does* still join transfers to messages — but only for the
time axis. `database.rs:3356-3364` (`get_total_counters`):

```rust
                    let total_transfers = crosschain_transfers::Entity::find()
                        .join(
                            JoinType::InnerJoin,
                            crosschain_transfers::Relation::CrosschainMessages.def(),
                        )
                        .filter(Expr::col(crosschain_messages::Column::InitTimestamp).lt(timestamp))
                        .filter(filter.transfers_condition())
                        .count(tx)
                        .await?;
```

That relation is the **composite** join
(`crosschain_transfers.rs:54-61`: `from = "(Column::MessageId, Column::BridgeId)"`,
`to = "(crosschain_messages::Column::Id, crosschain_messages::Column::BridgeId)"`).
(quoted block is `database.rs:3355-3363`.) `get_daily_counters`
(`database.rs:3395-3403`) is the same shape with a `[day_start, next_day_start)`
window.

### 6.5 The sixth dimension: `only_indexed_by_bridge` (the observability horizon)

**[F]** Request flag: `optional bool include_unindexed_chains`
(`interchain-indexer-proto/proto/v1/stats.proto:52` etc.), read as
`inner.include_unindexed_chains.unwrap_or(false)`
(`services/stats.rs:62`, `:97`, `:120`, `:209`, `:301`). **Default `false`, so the
restriction is ON by default.**

**[F]** `build_chain_bridge_filter` (`services/utils.rs:104-106`):

```rust
    let only_indexed_by_bridge = (!include_unindexed)
        .then(|| indexed_chains.configured_pairs(bridge_ids.as_deref()))
        .flatten();
```

**[F]** The rendered predicate (`filters.rs:72-107`) is a nested `OR` of:

- a **permissive arm** (messages L85-93): `bridge_id NOT IN (listed) AND
  dst_chain_id IS NOT NULL`; for transfers `bridge_id NOT IN (listed)` alone,
  because both token chain columns are `NOT NULL` so no NULL guard is needed
  (`filters.rs:161-168`). When the `listed` set is empty, the arm is the literal
  `TRUE` (`Expr::value(true)` at L89 / L165) because an empty `Condition::all()`
  renders to nothing.
- one disjunct **per listed bridge** (messages L94-105):
  `bridge_id = b AND src IN chains_b AND dst IN chains_b`. A NULL `dst` makes
  `dst IN (...)` NULL, so the row is excluded without an explicit NULL term.

Semantics per case (`IndexedChains::may_observe`, `indexed_chains.rs:94-101`, with
the truth table in its doc comment at L69-75):

| case | `may_observe` | effect on the read filter |
|---|---|---|
| `AllIndexed` (the enum's `Default`) | `true` | `configured_pairs` returns `None` → no restriction |
| bridge in map, chain in its set | `true` | row visible |
| bridge in map, chain not in its set | `false` | row hidden |
| bridge **absent** from map (removed from config) | `true` | **permissive** — rows stay visible |
| bridge in map with **empty** set | `false` | **all its rows hidden** |

The last two rows are deliberately opposite (ADR-004 Decision 5): *absent* means
decommissioned and must not have its history reinterpreted; *present-and-empty*
means misconfigured, and `server.rs:312-320` warns per bridge (plus a hard
`anyhow::ensure!` at L325-332 that rejects a config with bridges but zero pairs).

**[F]** The first row does **not** describe a no-bridges config, despite reading
that way. `server.rs` is the only production construction and always calls
`IndexedChains::from_bridges`, which builds `PerBridge` even from an empty
iterator; and its `ensure!` explicitly permits `bridges.is_empty()`. So an empty
config yields `PerBridge({})` → `configured_pairs` → `Some([])`, whose message
predicate is `(TRUE AND dst IS NOT NULL)` — not the unrestricted `None`.
`AllIndexed` is reachable only from the indexer's own tests and embedders. Stats'
resolver mirrors this: it returns an empty list for an empty `bridges` table, not
"no restriction".

**[F]** `has_unindexed_chain` on list responses is a **computed field**
(`indexed_chains.rs:235` `message_has_unindexed` / `:240`
`transfer_has_unindexed`, both built on `may_observe`), not a DB column. Nothing about "unindexed" is persisted per row (ADR-004
Decision 1).

### 6.6 Where the per-bridge indexed chain set comes from — and whether stats could derive it

**[F]** `interchain-indexer-server/src/server.rs:290-309`:

```rust
    // `load_bridges_from_file` is pure file IO and is hoisted above stats
    // wiring so the indexed-chain set below can be built from it. Nothing else
    // moves: the startup backfill a few lines down still runs before
    // `upsert_chains` / `upsert_bridges` / `upsert_bridge_contracts`, because a
    // DB-derived indexed-chain set would be stale exactly when backfill needs
    // it (the DB has no bridge_contracts rows yet on a fresh deployment).
    let bridges = load_bridges_from_file(&settings.bridges_config)?;
    …
    let indexed_chains = IndexedChains::from_bridges(bridges.iter().map(|b| {
        (
            b.bridge_id,
            b.contracts.iter().map(|c| c.chain_id).collect(),
        )
    }));
```

ADR-004 Decision 1 states it as a rule: *"It is derived from the **in-memory
configuration**, never from the `bridge_contracts` table."*

> **Correction to the pre-existing fact base.** An earlier note recorded that the
> per-bridge chain set "is persisted in the DB as `bridge_contracts` (+ `bridges`,
> `chains` tables) — so replicating it from stats is feasible." That is **wrong as
> stated.** The tables exist, but they are not the source of truth, and they cannot
> reconstruct the config-derived set faithfully. Three independent reasons, each
> verified:
>
> 1. **[F]** `upsert_bridge_contracts` (`database.rs:1914-1949`) is insert-or-update
>    only — `OnConflict` on `(bridge_id, chain_id, address, version)` updating
>    `abi`/`started_at_block`/`updated_at`. It **never deletes**. A chain removed
>    from a bridge's config keeps its row forever, so a DB-derived set is a
>    *superset* of the configured one, i.e. strictly more permissive.
> 2. **[F]** A bridge declared with **zero contracts** contributes zero
>    `bridge_contracts` rows. A DB-derived reconstruction therefore sees it as
>    *absent* (permissive) where the config-derived one sees *present-with-empty-set*
>    (restrictive) — exactly the distinction ADR-004 Decision 5 calls load-bearing.
>    `IndexedChains::from_pairs` carries the same warning in its doc comment
>    (`indexed_chains.rs:49-54`) about flat pair streams.
> 3. **[F]** `bridges.enabled` cannot disambiguate either. `upsert_bridges`
>    (`database.rs:929`) opens a transaction that first sets `enabled = false` on
>    **all** rows (L972-977) and then upserts the config bridges with their own
>    config `enabled` value (L980-995). So a bridge declared in config with
>    `enabled: false` and a bridge removed from config both end up
>    `enabled = false` — while `IndexedChains` deliberately includes disabled
>    bridges (`server.rs:298-303`:
>    *"`enabled` is an operational switch, not a statement about observability"*).
>
> **[I]** So stats could compute a *best-effort approximation* from
> `bridges` ⋈ `bridge_contracts`, but it would drift permissively and would get the
> contract-less-bridge case backwards. Faithful parity requires the same input the
> indexer uses — the bridges config file, or a new API/table that publishes the
> effective set.
> **[Q]** Which of those (share the config file with stats / add an indexer
> endpoint exposing `configured_pairs` / accept the approximation / declare the
> horizon out of scope for stats) is a design decision no code answers today.

### 6.7 The parity gap as it stood before the change

> The "stats" column below is the **pre-change** state. Section 6.8 gives the
> current one.

| dimension | interchain-indexer read API | stats before this work |
|---|---|---|
| focal `home_chain_id` | per request, 4-case truth table | one global `interchain_primary_id`; only `total_interchain_transfer_users` renders an OR |
| `counterparty_chain_ids` | supported, incl. `(None, Some(s))` within-set | **absent** |
| `src_chain_ids` / `dst_chain_ids` (directional) | outer-AND `IN` lists | **absent** (the `*_sent` / `*_received` chart *split* is a fixed proxy) |
| `bridge_ids` | outer-AND `IN` list on the row's own `bridge_id` | **absent** — `bridge_id` is never read |
| `only_indexed_by_bridge` (horizon) | ON by default | **absent** |
| transfers filtered on | transfer's own `token_src_chain_id` / `token_dst_chain_id` / `bridge_id` | **the joined message's** `src_chain_id` / `dst_chain_id` |
| transfers→messages join | composite `(message_id, bridge_id)` | **`ON t.message_id = m.id` only** |
| time bound | `init_timestamp < timestamp` (common) / day window (daily) | counters: none; line charts: batch range only |
| when the filter is applied | read time, per request | update time, per deployment |
| "totals" | filtered like everything else | 4 families unfiltered by design |

**[I]** The single most consequential item is the transfers row. Because a
transfer's token chains need not equal its message's chains (that is precisely
why the indexer has separate columns and why PR #1708 called it out), stats'
`total_interchain_transfers_sent` / `…_received` /
`new_transfers_sent_interchain` / `new_transfers_received_interchain` /
`total_interchain_transfer_users` answer a *different question* than the
indexer's transfer counters — not a coarser version of the same one.

### 6.8 What closed, what remains, and what a live run measured

**[F]** Every row of the 6.7 table is closed except the last two, which were
closed by *decision* rather than by change:

- **"when the filter is applied"** stays update-time. The filter is a deployment
  property, not a request parameter; the read path still never sees it. The
  consequence is the fingerprint mechanism (`d3ae7378`): a config change clears
  and recomputes rather than reinterpreting stored points.
- **"totals"** — the four previously-unfiltered families are now filtered, but
  `total_interchain_transfer_users` still carries **no** directional term even
  with a home chain set (a user is a user on either side of a route), and the
  transfer charts' directional term is on the transfer's own
  `token_src_chain_id`, not the message's `src_chain_id`. Both are deliberate;
  the second is the one place in the family where the two differ.

**[F]** Two genuine divergences remain, with one cause and opposite signs. The
indexer builds `only_indexed_by_bridge` from its config; stats resolves it from
`bridges LEFT JOIN bridge_contracts`, and `upsert_bridges` /
`upsert_bridge_contracts` never delete (`database.rs:1914`). A *removal* from
config therefore shrinks the indexer's set and leaves stats' set unchanged:

- **a bridge contract removed, bridge kept** ⇒ stats' set for that bridge is a
  superset ⇒ **stats admits rows the API now excludes**;
- **a whole bridge removed** ⇒ the API sees it as absent and the shared predicate
  admits its rows unconditionally (the permissive arm), while every message's
  `bridge_id` is an FK into `bridges` so **that arm is unreachable in stats** ⇒
  stats keeps testing its rows against the stale set ⇒ **stats admits fewer rows
  than the API**.

Neither is repaired by a forced recompute — it re-reads the same tables. Nothing
in the current deployment triggers either; both need a config deletion.

**[F]** Local end-to-end parity run (2026-08-19), stats and a dev
interchain-indexer over one frozen 92,976-message / 72,733-transfer database
(Omnibridge on chains 1+100, Avalanche ICTT on 43114 + 8021 + 68414, plus seven
chains that appear in messages but that no bridge indexes):

- the horizon sets agreed exactly — the indexer's config-derived
  `indexed_chain_ids` from `GET /api/v1/interchain/bridges` versus stats'
  DB-derived resolution — including the two chains added through env vars;
- for six filter shapes (unfiltered, focal, focal+counterparty, within-set
  conjunction, directional+bridge, horizon off) the indexer's own list endpoints
  and stats' counters returned identical totals, and ~22,500 point-level
  comparisons of every counter, daily series, cumulative series and
  week/month/year rollup against hand-written SQL found no discrepancy;
- the horizon excluded 824 of 92,976 messages, so it is load-bearing on real data
  rather than a no-op;
- the whole 131-chart first computation took **1.9 s**, which retires the
  unattended-first-run cost concern at this volume. Plans: the focal
  `(src = N OR dst = N)` term is index-driven (BitmapOr over
  `crosschain_messages_src_chain_ts_idx` / `…_dst_chain_ts_idx`); the horizon
  disjunction is a heap filter with no index support, so a horizon-only
  configuration is a sequential scan (35 ms here) that grows linearly with the
  table.

**[I]** Two things the live data could **not** exercise, and which therefore rest
on fixtures alone: no message id is reused across bridges in this dataset (so the
composite-join fix is indistinguishable from the `message_id`-only join here),
and no row has a NULL destination. Both were checked by inserting synthetic rows
— the NULL-destination row and a row addressed to an unindexed chain were both
correctly excluded by the horizon, and by the incremental (non-clearing) update
path as well as the full one.

---

## 7. Testing: Harness, Fixtures, and What They Encode

**[F]** Two entry points, both in `stats/src/tests/simple_test.rs`:

- `simple_test_chart_interchain<C>(test_name, expected, interchain_primary_id)`
  (L138-157) → `simple_test_chart_inner` with `Mode::Interchain` (L151).
- `simple_test_counter_interchain<C>(test_name, expected, update_time, interchain_primary_id)`
  (L480-498) → `simple_test_counter_inner` with `Mode::Interchain` (L493).

Both run the update twice — once with `force_full: true`, once with `false` —
asserting the same expected output each time (`simple_test_chart_inner` at L220;
the two update+assert passes at L253-276).
Update time is pinned to `2023-03-01T12:00:00Z`.

**[F]** DB setup: `init_db_all_interchain` (`tests/init_db.rs:122`) creates a
stats DB from the real `migration::Migrator` plus an "interchain" DB from a
**locally hand-written migrator** (`tests/init_db.rs:11-83`), introduced with an
explicit TODO (L5-9): *"The interchain indexer DB schema is not in the current
branch, so we define a local migrator here that creates only the
crosschain_messages table for tests."* The DDL it creates:

```sql
                    CREATE TABLE crosschain_messages (
                        id BIGSERIAL PRIMARY KEY,
                        init_timestamp TIMESTAMPTZ NOT NULL,
                        src_chain_id BIGINT NOT NULL,
                        dst_chain_id BIGINT NOT NULL,
                        src_tx_hash BYTEA,
                        dst_tx_hash BYTEA
                    )
```

```sql
                    CREATE TABLE crosschain_transfers (
                        id BIGSERIAL PRIMARY KEY,
                        message_id BIGINT NOT NULL REFERENCES crosschain_messages(id),
                        sender_address BYTEA,
                        recipient_address BYTEA
                    )
```

**[F]** Divergences from the production schema (§4), all consequential:

| production | test harness |
|---|---|
| `PRIMARY KEY (id, bridge_id)` | `id BIGSERIAL PRIMARY KEY` — **no `bridge_id` column at all** |
| `dst_chain_id BIGINT NULL` | `dst_chain_id BIGINT NOT NULL` |
| `init_timestamp TIMESTAMP` | `init_timestamp TIMESTAMPTZ` |
| transfers have `bridge_id`, `token_src_chain_id`, `token_dst_chain_id`, `index`, amounts, token addresses, `type` | transfers have only `id`, `message_id`, `sender_address`, `recipient_address` |
| composite FK `(message_id, bridge_id)` + `UNIQUE (message_id, bridge_id, index)` | single-column FK `message_id → crosschain_messages(id)` |
| `status`, `native_id`, `payload`, `stats_processed`, … | absent |

**[I]** The `TIMESTAMPTZ` vs `TIMESTAMP` difference means `init_timestamp::date`
and `init_timestamp::timestamp` are session-timezone-dependent casts in tests and
plain no-ops/date-truncations in production. Tests happen to pass because the test
harness inserts `NaiveDateTime` values and the CI session timezone is UTC. **[Q]**
Worth confirming with an `EXPLAIN`/session-`SET TimeZone` experiment whether any
day-boundary bucketing differs between the two schemas; not resolvable from source
alone.

**[F]** Fixture shape (`stats/src/tests/mock_interchain.rs`): 21 messages and 41
transfers over Dec 2022 / Jan 2023 / Feb 2023, with deliberate date holes.
`mock_rows()` (L19-48) is a `Vec<(init_timestamp, src_chain_id, dst_chain_id,
has_src_tx, has_dst_tx, num_transfers)>`. Chain ids used: **1, 2, 3 only**.
Transfers are generated per message (`fill_mock_interchain_data:116-129`) with
`sender_idx = (transfer_id - 1) % 8` and `recipient_idx = (transfer_id + 2) % 8`,
i.e. exactly 8 distinct 20-byte addresses — the comment at L112-113 says so,
which is why `totalInterchainTransferUsers` expects `"8"`.

**[F]** Fixture-derived expectations (all re-derived from `mock_rows()` and
confirmed to match the asserted values):

| counter | unfiltered | `primary_id = Some(1)` |
|---|---|---|
| `totalInterchainMessages` | 21 | not tested (ignores filter) |
| `totalInterchainMessagesSent` | 15 | 11 |
| `totalInterchainMessagesReceived` | 13 | 6 |
| `totalInterchainTransfers` | 41 | not tested (ignores filter) |
| `totalInterchainTransfersSent` | 37 | 23 |
| `totalInterchainTransfersReceived` | 20 | 6 |
| `totalInterchainTransferUsers` | 8 | **not tested at all** |

**[F]** What the fixtures encode about filtering: only shapes A and B with a
single chain id, on a single implicit bridge. What they **cannot** encode, because
the columns do not exist in the test schema:

- any `bridge_id` behaviour, including the composite-join fan-out of §3.4;
- any divergence between a transfer's token chains and its message's chains;
- a NULL `dst_chain_id` (the "destination unknown" case the indexer's horizon
  filter treats specially);
- more than one bridge, so `only_indexed_by_bridge` has nothing to bite on.

**[F]** `total_interchain_transfer_users` has **no filtered test case** — the only
call is `simple_test_counter_interchain::<TotalInterchainTransferUsers>("update_total_interchain_transfer_users", "8", None, None)`
(`total_interchain_transfer_users.rs:90-96`). It is the one counter whose
predicate shape (C) is unique, and it is the one with no coverage of that shape.

**[F]** Only three of the eight line-chart families have a `primary_1` variant of
their weekly/monthly/yearly tests: `new_messages_sent_interchain` (all four
resolutions), `new_messages_received_interchain` (base only),
`messages_growth_sent_interchain` / `messages_growth_received_interchain` (base
only). The transfer line charts have unfiltered tests only.

**[F]** Interchain charts appear in `stats-server/tests/it/` only via
`linked_stats.rs`, which mocks `/api/v1/pages/interchain/main` responses with
hand-written counter values. There is no end-to-end interchain integration test
against a real interchain schema.

---

## 8. Invariants

**[F]** Verified invariants of the current implementation:

1. `interchain_primary_id` is **update-time only**. Every read-path
   `UpdateParameters` sets it to `None` (`types.rs:62`, `:91`).
2. All interchain statements use `UsePrimaryDB`, i.e. `cx.indexer_db`. No
   interchain chart touches `second_indexer_db`.
3. Every filtered statement binds the chain id as `$1` and nothing else, so the
   range filter always starts at `$2` when filtered and `$1` when not.
4. `crosschain_messages.init_timestamp` is the only time axis for every
   interchain chart, including all transfer charts and all "received" charts.
5. `get_min_block_interchain` is a constant, so block-based re-update
   invalidation is inert in interchain mode.
6. `get_min_date_interchain` is unfiltered — the time domain of a filtered chart
   is still bounded by the *global* earliest message.
7. The two `*_growth_*` charts never build SQL; they derive from their `New*Int`
   counterparts and inherit whatever filtering those applied.
8. Interchain groups are registered in `RuntimeSetup` unconditionally; only
   config gates them.

**[F]** Invariants on the indexer side that stats must not break if it ports the
predicate:

9. Directional and bridge terms go in the **outer AND**, never inside the focal
   `OR` (`filters.rs:63-71`, with the in-code warning).
10. `only_indexed_by_bridge` is one nested `OR` appended to the outer AND, with an
    explicit permissive arm; flattening it admits rows from unselected bridges
    (in-code warning at `filters.rs:73-77`).
11. `(None, Some(set))` is a conjunction ("within-set"), not a focal OR.
12. Messages filter on message chain columns; transfers filter on the transfer's
    **own** token chain columns and **own** `bridge_id`.
13. Nothing about "unindexed" is persisted per row; it is always computed from
    `IndexedChains` (ADR-004 Decision 1).

## 9. Failure Modes / Observability

**[F]** Errors from these queries surface as `ChartError::IndexerDB` (min date /
min block) or as a group-level `tracing::error!("error during updating group: {}")`
in `UpdateService::update_group` (`update_service.rs:450-456`); a failing group is
logged and skipped, not fatal.

**[F]** Per-chart update metrics exist (`CHART_FETCH_NEW_DATA_TIME`,
`local_db/mod.rs:222-228`), keyed by `ChartProps::key()`. There is **no metric or
log line that records the active `interchain_primary_id`**, so a
misconfigured/changed filter is not observable from telemetry.

**[I]** The characteristic symptom of the parity gap is not an error: it is stats
totals that are **greater than or equal to** the indexer's default `/stats/common`
totals (stats has no horizon filter, no bridge filter, and no time bound), plus
transfer counts that can be either higher or lower depending on how often a
transfer's token chains differ from its message's chains.

## 10. Known Limitations and Change Triggers

**[F]** Limitations (each traceable to a specific line):

- One filter value per deployment; no per-request filtering (`types.rs:62`).
- Changing the value does not invalidate stored data
  (`settings.rs:100` TODO; constant `min_indexer_block`).
- 4 of 15 chart families ignore the filter.
- `bridge_id` is invisible to stats; the transfers join is not bridge-qualified.
- Transfer filtering goes through the message, not the transfer.
- No timestamp bound on counters.
- No index supports `src_chain_id` / `dst_chain_id` predicates.
- The test schema cannot express the columns the parity work needs.

**Change triggers — this note must be revisited when:**

- `stats/src/tests/init_db.rs` stops hand-rolling the interchain schema (the TODO
  at L5-9 anticipates depending on the real `interchain_indexer_migration::Migrator`);
- any new column of `crosschain_messages` / `crosschain_transfers` starts being
  read by stats, or the indexer changes those tables' nullability or PK;
- `Settings` grows a second interchain filter field, or `interchain_primary_id`
  stops being the only one;
- `UpdateParameters::query_parameters` stops forcing the filter to `None` (that
  would move filtering to read time and invalidate most of §3);
- `ChainBridgeFilter` gains or loses a dimension, or the focal truth table changes;
- `IndexedChains` stops being config-derived (§6.6 would need rewriting);
- interchain charts start consuming the indexer's projected `stats_*` tables
  instead of re-aggregating the canonical ones.

## 11. Open Questions

Questions 1-4 were settled by the implementation; the answers are recorded here
rather than deleted, because each one shaped a decision.

1. **Answered.** Yes — the dev instance this note was written against already
   hosts two bridges (AMB/Omnibridge and Avalanche ICTT), so the
   non-bridge-qualified join was a live over-counting risk. The join is now
   composite. No message id is in fact reused across the two bridges in that
   dataset, so the bug never fired there; the fixture carries a deliberate
   collision instead (§6.8).
2. **Answered — yes, all of them.** Every counter and chart follows the filter,
   and a coverage test asserts it, so a new chart cannot quietly opt out.
3. **Answered — yes.** The horizon is part of the parity target and is on by
   default, matching the API's default.
4. **Answered — `bridges ⋈ bridge_contracts`**, resolved per update cycle from
   the indexer DB. No config file and no new endpoint. The residual inaccuracy
   this accepts is the deleted-bridge case in §6.8.
5. **[Q]** Does the `TIMESTAMPTZ` (test) vs `TIMESTAMP` (production) column type
   change any day-boundary bucketing? Needs a DB experiment; not answerable from
   source.
6. **[Q]** Should stats' counters gain the indexer's `init_timestamp < now` bound,
   or is counting future-dated rows acceptable/intended?
7. **[Q]** Is `total_interchain_transfer_users` meant to be a *unique-address*
   count within the focal slice, or a global one? The indexer's comparable
   `/stats/chains` unique-user counts were explicitly left global/exact and outside
   the filtering model (PR #1708 follow-ups), so there is no parity target to copy.

## 12. Related Notes

- `stats/.memory-bank/gotchas.md` — "`IndexerMigrations` Is Only Ever Queried For
  Blockscout/Zetachain Mode", "The Second (CCTX) Indexer DB Only Connects in
  `Zetachain` Mode", "Trailing Line-Chart Points Are Marked Approximate By Design".
- `stats/.memory-bank/rules/database.md` — per-mode `read::*` submodule dispatch.
- `interchain-indexer/.memory-bank/research/stats-subsystem.md` — the indexer's own
  stats endpoints, projection tables, and endpoint-by-endpoint calculation rules.
- `interchain-indexer/.memory-bank/research/stats-projection.md` — projection
  eligibility mechanics.
- `interchain-indexer/.memory-bank/adr/004-…` — Decisions 1 and 5 (the horizon and
  the "config change never reinterprets history" rule).
- `interchain-indexer/.memory-bank/adr/002-primary-chain-filtering.md` — the
  indexer's **write-time** per-bridge `home_chain_id` / `process_unknown_chains`
  filtering, which is independent of everything in §6.
