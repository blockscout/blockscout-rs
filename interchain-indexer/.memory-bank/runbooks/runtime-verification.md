# Runtime Verification

Read this when you have a live indexer running against a real database (production
or staging) and want to confirm behavior directly against the tables — not
against a design doc or a reviewer's argument, against what actually landed
in the rows.

This runbook currently covers the stats subsystem's observability-horizon
eligibility rule and asset-identity union-find merge. Design rationale for
that area lives in
[ADR-004](../adr/004-stats-observability-horizon-and-asset-union-find.md) and,
for the two-asset-column / per-transfer-linkage model added on top of it, in
[ADR-011](../adr/011-cross-asset-edges-and-per-transfer-linkage.md); this
document does not re-argue either, it only tells you what to run and how to
read the output. If a query's expected result surprises you, read the ADR
section it references before assuming a bug.

**Every query below is a plain `SELECT`. None of them write, and none of
them lock anything beyond what Postgres takes for an ordinary read. They are
safe to run against a live database at any time, repeatedly, including while
the indexer is writing to the same tables.** Keep that property for any
query you add here — this file is meant to grow, not just be read.

## Conventions for adding an entry

This runbook is expected to gain siblings and grow new entries as the
service develops. Keep additions in the same shape so it stays navigable:

- **Canary or diagnostic?**
  - A **canary** is cheap, has one fixed expected answer (typically zero
    rows or all-zero counts), and costs nothing to run on a schedule — after
    every deploy, daily, or on a dashboard — without needing a judgment call
    to interpret. Put a new binary pass/fail structural check here.
  - A **diagnostic** takes more reading to interpret, is reached for only
    once a canary fires or you want to confirm one specific piece of
    behavior, and is not meant to be monitored blindly. Put anything that
    needs classification, a follow-up query, or domain judgment here.
- **Per-entry shape.** Every entry states, in this order: what it checks,
  why it matters, the query (copy-paste runnable; literal placeholders are
  spelled out and clearly marked, e.g. `__SRC_ASSET__`, never bare `:x`
  psql-style syntax), the expected result, what a deviation means, and the
  next step.
- **Run canaries first.** Only reach for a diagnostic once you know which
  question you're asking.
- A new query about a different part of the service (not stats/observability)
  is still a canary or a diagnostic by the same test above — add it as a new
  lettered entry in the appropriate section rather than starting a parallel
  scheme.

## Two things every query on this page assumes

**1. `bridge_contracts` is a proxy for the runtime membership set, not the
set itself.** The indexer's eligibility decisions (`IndexedChains::may_observe`)
are made from the **in-memory configuration** loaded from `bridges.json`,
never from this table. `bridge_contracts` is populated by
`upsert_bridge_contracts`, which the server calls once at startup — and,
on a fresh database, **after** `stats.backfill_on_start` has already run
(`interchain-indexer-server/src/server.rs`: startup backfill runs before the
upsert). So on a database that has only ever seen a single
`BACKFILL_ON_START=true` run and nothing else, `bridge_contracts` may still
be empty (or short of the real set) at the moment a diagnostic query like D
or E is evaluated against it — the table catches up once the server
finishes starting, but a query run in that exact window undercounts
"indexed" and over-labels rows as unindexed-related.

There is a second, permanent asymmetry worth knowing about, in the opposite
direction: `upsert_bridge_contracts` only inserts and updates
(`ON CONFLICT ... DO UPDATE`) — there is no corresponding delete anywhere in
the codebase for a contract removed from `bridges.json`. So if a chain is
ever removed from a bridge's contract list in config, its row simply stays
in `bridge_contracts` forever, silently overstating what the running
indexer actually observes. Neither direction is a bug — the ADR is explicit
that the DB table is deliberately not the source of truth (ADR-004,
Decision 1) — but both mean: **if `bridge_contracts` disagrees with what you
know `bridges.json` says, believe `bridges.json`.** For any query below that
joins against `bridge_contracts` (D, E), treat a surprising result as a
prompt to check the actual config before treating it as a projection defect.

**2. A successful asset merge is nearly invisible in the database.** The
losing `stats_assets` row is deleted as part of the merge (that is the
design — no `merged_into` pointer, no read path has to dereference
anything). So a healthy merge leaves no "before" state to compare against in
SQL; you cannot query your way to "how many merges happened" from table
contents alone. Use the metrics instead:

| Metric | Type | Labels | What it tells you |
| --- | --- | --- | --- |
| `interchain_indexer_stats_asset_merges_total` | counter | `outcome` (`merged`, `refused_chain_collision`, `refused_token_on_chain`) | Every merge attempt, by outcome, for `mirror` transfers only. Either refused outcome rising is query A/B territory; both mean the same diagnosis — bad token data, or a converting route the indexer declared `mirror`. |
| `interchain_indexer_stats_asset_merge_repointed_transfers` | histogram | none | `crosschain_transfers` rows repointed per successful merge — how expensive merges are getting. |
| `interchain_indexer_stats_edge_rescaled_fold_total` | counter | `mode` (`scaled_up`, `scaled_down`, `unscaled_unknown_decimals`, `unscaled_overflow`) | How a merge combined two edges' `cumulative_amount` when decimals differed or were missing. |
| `interchain_indexer_stats_edge_mixed_amount_side_total` | counter | none | A merge folded two edges that had different `amount_side` (source vs. destination) — result is approximate. |
| `interchain_indexer_stats_edge_decimals_conflict_total` | counter | none | Non-merge counting-path skip: a transfer's amount could not be safely folded into an existing edge because decimals changed. Distinct from a merge refusal. |
| `interchain_indexer_stats_transfers_deferred_total` | counter | `reason` (`identity_incomplete`, `awaiting_confirmation`, `linkage_unknown`, `conversion_endpoint_unresolved`, `amount_side_missing`) | Deferral **events**, not distinct rows — a row re-increments every time its canonical key is flushed again and still isn't eligible. The last three reasons are [ADR-011](../adr/011-cross-asset-edges-and-per-transfer-linkage.md) additions — see the unclassified-rows query (H) for `linkage_unknown` specifically. |
| `interchain_indexer_stats_asset_linkage_contradiction_total` | counter | `kind` (`conversion_self_asset`, `cross_asset_edge_collapsed`) | [ADR-011](../adr/011-cross-asset-edges-and-per-transfer-linkage.md): an indexer's declared linkage disagrees with the resolved asset graph. Never fatal, always worth investigating — see the gotcha "A `conversion` Transfer's Two Endpoints Are Never Merged". |
| `interchain_indexer_stats_transfer_asset_linkage_unset_total` | counter | none | A transfer reached the write chokepoint with `asset_linkage` left `NotSet` by its indexer — should be zero always; see the gotcha "The `..Default::default()` Omission Silently Defers Every Transfer From An Indexer". |

(Verified against `interchain-indexer-logic/src/stats/metrics.rs`.)

**There is deliberately no exact invariant of the form "sum of
`stats_asset_edges.transfers_count` equals the number of counted
transfers."** Two paths mark a transfer `stats_processed` without it ever
contributing to an edge: a refused merge (identity stays unknown, both
`src_stats_asset_id` and `dst_stats_asset_id` are `NULL`) and the decimals
conflict on the counting path (identity is known, both columns are set, but
the amount is skipped). SQL cannot tell these two apart from a transfer row
alone in the aggregate case — that's exactly what
`STATS_EDGE_DECIMALS_CONFLICT_TOTAL` and the two refusal outcomes quantify. A
gap between processed transfers and summed edge counts is expected; don't
chase it as a bug by itself — check those counters first.

**The diagnostic reading that makes queries D and A interpretable, on
`crosschain_transfers`:**

- `asset_linkage IS NULL` → the indexer has not yet stated this transfer's
  linkage. Deferred unconditionally, `stats_processed` stays `0`. Not the
  same state as either row below — see query H for the operational query
  that surfaces these specifically.
- `src_stats_asset_id IS NULL AND dst_stats_asset_id IS NULL AND
  stats_processed > 0` → identity is genuinely unknown or ambiguous (a
  refused merge is the only remaining way to reach this for a `mirror`
  transfer, once the row has actually been processed);
- **both columns set** together with `stats_processed > 0` and no matching
  contribution in `stats_asset_edges` → identity is known, but this
  transfer's amount specifically was not counted (the decimals-conflict
  skip, or — for a `conversion` transfer — the `amount_side_missing`
  deferral, which additionally leaves `stats_processed = 0`, so check that
  column too before concluding "counted but skipped").
- **both columns set, unequal** (`src_stats_asset_id <> dst_stats_asset_id`)
  → a `conversion` transfer that resolved its two endpoints to two different
  assets, exactly as designed. Do not read this as a split-asset defect —
  that is what canary A's `asset_linkage = 'mirror'` qualification exists to
  exclude.

Both `src_stats_asset_id IS NULL AND dst_stats_asset_id IS NULL` with
`stats_processed = 0` is not evidence of anything by itself — it is simply
the normal state of a deferred or not-yet-projected transfer that hasn't been
decided on yet (see query D for the full breakdown of why a row might still
be at `stats_processed = 0`). Only once `stats_processed > 0` does both
columns `NULL` mean a refused merge specifically; reading NULL alone as
"identity conflict" will misclassify ordinary backlog as a defect.

Processed cases leave the row `stats_processed`'d so projection does not
re-warn on it every cycle — this is deliberate, not a stuck row. A
`linkage_unknown` row is the one deferral that stays at `stats_processed = 0`
indefinitely until an operator intervenes (query H) — it is not self-healing
the way `identity_incomplete` / `awaiting_confirmation` are.

---

## Canaries — run routinely

### C. Hard invariants (all five columns must read 0)

**What it checks:** five structural guarantees that hold regardless of
config or data completeness — nothing in the design should ever produce
double-counting or an orphaned row.

**Why it matters:** these are invariants of the counting/identity machinery
itself (ADR-004 Decision 3), not of any particular chain or bridge
configuration. A nonzero value here means a defect in the shared code path,
not a data quirk.

```sql
SELECT (SELECT count(*) FROM crosschain_transfers WHERE stats_processed > 1) AS transfers_over_counted,
       (SELECT count(*) FROM crosschain_messages  WHERE stats_processed > 1) AS messages_over_counted,
       (SELECT count(*) FROM stats_assets a
          WHERE NOT EXISTS (SELECT 1 FROM stats_asset_tokens t WHERE t.stats_asset_id = a.id)) AS orphan_assets,
       (SELECT count(*) FROM stats_asset_edges e
          WHERE NOT EXISTS (SELECT 1 FROM stats_assets a WHERE a.id = e.src_stats_asset_id)) AS dangling_edges_src,
       (SELECT count(*) FROM stats_asset_edges e
          WHERE NOT EXISTS (SELECT 1 FROM stats_assets a WHERE a.id = e.dst_stats_asset_id)) AS dangling_edges_dst;
```

**Expected result:** `0 | 0 | 0 | 0 | 0`.

**What each column means if nonzero:**
- `transfers_over_counted` / `messages_over_counted` — a row was counted more
  than once. Every write to `stats_processed` is `+1` gated behind
  `stats_processed = 0` (`interchain-indexer-logic/src/stats/projection.rs`),
  so this should be structurally impossible; if it fires, the guard itself
  is broken.
- `orphan_assets` — a `stats_assets` row with no `stats_asset_tokens` linked
  to it. Every asset is created together with at least one token link in the
  same transaction; an orphan means an asset row survived a merge or
  creation without its token rows, or the merge's delete-the-loser step ran
  against the wrong id.
- `dangling_edges_src` / `dangling_edges_dst` — a `stats_asset_edges` row
  pointing at a `stats_assets` id that no longer exists, on the source or
  destination side respectively. Both `src_stats_asset_id` and
  `dst_stats_asset_id` have an `ON DELETE CASCADE` foreign key to
  `stats_assets(id)`, so either column reading nonzero should be enforced by
  the schema itself; seeing this means either the FK is missing on your
  schema version or something bypassed it (e.g. a manual `DELETE`). Checking
  both columns separately matters post-[ADR-011](../adr/011-cross-asset-edges-and-per-transfer-linkage.md):
  a `conversion` edge's two sides can be deleted independently (they are two
  different `stats_assets` rows), so a single combined check could mask one
  side dangling while the other is fine.

**What to do next:** any nonzero value here is a "stop and investigate"
signal, not a "keep an eye on it" one. Capture the specific row ids (`SELECT
id FROM crosschain_transfers WHERE stats_processed > 1 LIMIT 20`, etc.) and
treat it as a bug report against the merge/projection code, not against
input data.

### A. Split-asset detector (the headline check — must return zero rows for `mirror` transfers)

**What it checks:** any pair of `mirror`-linkage transfer endpoints whose
tokens are currently mapped to *different* `stats_assets`. This is the
literal definition of a split asset — the exact defect the union-find
asset-merge design (ADR-004 Decision 2) exists to eliminate, for the
lock/mint bridges that design covers.

**Why it matters:** without eager union-find merging, this query can return
real, permanent splits — a transfer landing on two components silently
skipped forever (see ADR-004 Context for the production case this
generalizes from). Under the current design a merge should join the two
components as soon as such a transfer is observed, so this should always
come back empty on any database that has been fully projected under it.

**Why `AND t.asset_linkage = 'mirror'` is load-bearing, not optional
filtering** ([ADR-011](../adr/011-cross-asset-edges-and-per-transfer-linkage.md)):
a `conversion` transfer's two endpoints are *supposed* to map to two
different `stats_assets` — that is the entire point of the cross-asset edge
model, not a defect. Without this clause, the first xDai transfer ever
projected would make this canary fire permanently, turning the split-asset
detector into constant noise instead of a signal. If you are running this
query against a database predating ADR-011 (no `asset_linkage` column), drop
the clause; if `asset_linkage` exists, never drop it.

**Why the query also requires `t.stats_processed > 0`:** a freshly flushed
transfer that projection has not reached yet can already have both its token
endpoints mapped to two different `stats_assets` by *earlier* transfers —
and it is precisely this not-yet-projected transfer whose merge would join
those two components. Without the predicate, this ordinary, momentary lag
between flush and projection reads as a false-positive split. Do not
"simplify" this predicate away: dropping it reintroduces that false positive
on any database with normal write/projection concurrency, not just a broken
one.

```sql
SELECT t.bridge_id,
       t.token_src_chain_id, encode(t.token_src_address,'hex') AS src_token, s.stats_asset_id AS src_asset,
       t.token_dst_chain_id, encode(t.token_dst_address,'hex') AS dst_token, d.stats_asset_id AS dst_asset,
       count(*) AS transfers, min(t.id) AS example_transfer
FROM crosschain_transfers t
JOIN stats_asset_tokens s ON s.chain_id = t.token_src_chain_id AND s.token_address = t.token_src_address
JOIN stats_asset_tokens d ON d.chain_id = t.token_dst_chain_id AND d.token_address = t.token_dst_address
WHERE s.stats_asset_id <> d.stats_asset_id
  AND t.stats_processed > 0
  AND t.asset_linkage = 'mirror'
GROUP BY 1,2,3,4,5,6,7
ORDER BY transfers DESC;
```

**Expected result:** zero rows.

**What a row means:** a transfer whose two endpoints are still mapped to two
different assets. This is not automatically a bug — see diagnostic B, which
is the required next step for every row this returns. Do not treat a row
here as confirmed evidence of a regression until B has been run for that
`src_asset` / `dst_asset` pair.

**What to do next:** for every distinct `(src_asset, dst_asset)` pair
returned, run diagnostic B with those two ids substituted in.

---

## Diagnostics — run when a canary fires, or to confirm specific behaviour

### B. If A returns rows — legitimate refusal or a bug?

**What it checks:** whether the two assets from a row in A each hold *the
same* token on some shared chain, or *different* tokens on the same chain.
A merge is correctly refused only in the second case — a `stats_asset` can
hold at most one token per chain, so joining two components that would put
two different token addresses on the same chain under one asset is the one
conflict that genuinely cannot be resolved automatically.

**Why it matters:** this is the test that tells you whether a row from A is
expected (a real, pre-existing data problem the merge correctly declined to
paper over) or an actual defect in the merge logic (a pair that should have
merged silently failed to). This diagnostic does not need its own
`stats_processed` qualification — it takes an already-returned `(src_asset,
dst_asset)` pair from A as given, and A's `t.stats_processed > 0` predicate
is what already ruled out the transient flushed-but-not-yet-projected case
before the pair ever reaches here.

Replace both occurrences of `__SRC_ASSET__` and `__DST_ASSET__` below with
the literal integers from A's `src_asset` / `dst_asset` columns for the row
you're investigating (find-and-replace before pasting, or edit in place) —
they are placeholders, not runnable SQL as written.

```sql
SELECT chain_id,
       max(CASE WHEN stats_asset_id = __SRC_ASSET__ THEN encode(token_address,'hex') END) AS token_in_a,
       max(CASE WHEN stats_asset_id = __DST_ASSET__ THEN encode(token_address,'hex') END) AS token_in_b
FROM stats_asset_tokens
WHERE stats_asset_id IN (__SRC_ASSET__, __DST_ASSET__)
GROUP BY chain_id
HAVING count(*) > 1;
```

**Expected result / what it means:**
- **A row is returned** (a chain where the two assets disagree on the token
  address) → the refusal is **correct**. `interchain_indexer_stats_asset_merges_total{outcome="refused_chain_collision"}`
  should have incremented for this pair. The underlying problem is upstream
  data — a token address was almost certainly recorded against the wrong
  chain for one of the two components at some point. Fix: verify the token
  address recorded per chain for both components, correct the source data,
  then reset the affected transfers' `stats_processed` to `0` for
  re-projection (see `.memory-bank/gotchas.md`, "Stats Asset Mapping
  Conflicts Merge; Only Same-Chain Collisions Skip"). For local/staging
  environments a clean reindex is usually simpler than surgical repair.
- **No row is returned** (empty result) → the merge **should have
  happened** and did not. This is a genuine defect — the refusal condition
  fired without an actual same-chain collision, or the merge failed
  partway before validation. Worth a bug report with the specific
  `src_asset`/`dst_asset` pair and the example transfer id from query A.

### D. Deferred transfers, classified by reason

**What it checks:** every transfer not yet counted (`stats_processed = 0`),
bucketed by *why* it isn't counted yet, using the same observability-horizon
logic the projection code applies (approximated here via the
`bridge_contracts` proxy — see the caveat above).

**Why it matters:** most of the reasons below are expected, ordinary
backlog that the observability-horizon design leaves alone by construction
(ADR-004 Decision 1). Only two outcomes are worth acting on, and this query
is how you tell them apart from the harmless ones without reading source.

```sql
WITH indexed AS (SELECT DISTINCT bridge_id, chain_id FROM bridge_contracts)
SELECT t.bridge_id, m.status,
  CASE
    WHEN t.token_src_address IS NULL AND i_src.chain_id IS NOT NULL THEN 'awaiting src token, chain indexed'
    WHEN t.token_dst_address IS NULL AND i_dst.chain_id IS NOT NULL THEN 'awaiting dst token, chain indexed'
    WHEN t.token_src_address IS NULL OR  t.token_dst_address IS NULL THEN 'one-sided, peer unindexed'
    WHEN m.dst_chain_id IS NULL                                      THEN 'destination unknown'
    WHEN m.status <> 'completed' AND i_dst_m.chain_id IS NOT NULL     THEN 'awaiting confirmation, dst indexed'
    ELSE 'COMPLETE AND CONFIRMED - real backlog'
  END AS reason,
  count(*), min(t.id) AS example
FROM crosschain_transfers t
JOIN crosschain_messages m ON m.id = t.message_id AND m.bridge_id = t.bridge_id
LEFT JOIN indexed i_src   ON i_src.bridge_id   = t.bridge_id AND i_src.chain_id   = t.token_src_chain_id
LEFT JOIN indexed i_dst   ON i_dst.bridge_id   = t.bridge_id AND i_dst.chain_id   = t.token_dst_chain_id
LEFT JOIN indexed i_dst_m ON i_dst_m.bridge_id = t.bridge_id AND i_dst_m.chain_id = m.dst_chain_id
WHERE t.stats_processed = 0
GROUP BY 1,2,3 ORDER BY 4 DESC;
```

**How to read each reason:**

| Reason | Status | Action needed |
| --- | --- | --- |
| `awaiting src token, chain indexed` | expected | none — will resolve once the source-chain event is observed |
| `awaiting dst token, chain indexed` | expected | none — will resolve once the destination-chain event is observed |
| `destination unknown` | expected | none — `dst_chain_id IS NULL` defers unconditionally; there is no chain to test yet |
| `awaiting confirmation, dst indexed` | expected | none — will resolve once the destination chain confirms |
| `one-sided, peer unindexed` | **watch** | **must not grow.** A transfer whose missing endpoint's chain is *not* indexed should be committed to what is known and counted on its next projection pass, not deferred. If this bucket is nonzero and climbing rather than draining, projection is not sweeping these rows — treat as a bug. |
| `COMPLETE AND CONFIRMED - real backlog` | **watch** | This bucket also legitimately includes rows that are countable *right now* per the "destination chain unindexed → count regardless of status" rule but simply haven't hit a projection pass yet — so a small, steady trickle here is normal lag. If it is large or growing without bound, projection has fallen behind or missed something; check indexer health and maintenance-cycle cadence before assuming corruption. |

**Remember the proxy caveat:** on a freshly migrated database that has only
run one `BACKFILL_ON_START=true` pass, `bridge_contracts` may not yet
reflect the real config (upsert runs after backfill), which will make this
query over-classify rows as "chain indexed" cases (deferring) when they
were, in fact, evaluated against the real config correctly. Re-run this
query after the server has been up for a few minutes (past the startup
upsert) if the counts look inconsistent with what `bridges.json` says.

### E. Unindexed-chain edges — what the observability horizon retains

**What it checks:** `stats_asset_edges` rows where at least one side of the
edge is on a chain not indexed for that bridge. Under the older,
completeness-only rule, rows like this could not exist — a permanently
one-sided transfer was deferred forever instead of committed. They are
exactly what the (currently always-effectively-on-for-projection,
opt-in-for-reads) `include_unindexed_chains` behaviour is retaining.

**Why it matters:** confirms the "commit to what is known" half of the
eligibility rule (ADR-004 Decision 1) is actually writing rows, not just
theoretically eligible to.

```sql
WITH indexed AS (SELECT DISTINCT bridge_id, chain_id FROM bridge_contracts)
SELECT e.bridge_id, e.src_chain_id, e.dst_chain_id,
       (i_src.chain_id IS NULL) AS src_unindexed,
       (i_dst.chain_id IS NULL) AS dst_unindexed,
       e.transfers_count, e.amount_side, e.decimals
FROM stats_asset_edges e
LEFT JOIN indexed i_src ON i_src.bridge_id = e.bridge_id AND i_src.chain_id = e.src_chain_id
LEFT JOIN indexed i_dst ON i_dst.bridge_id = e.bridge_id AND i_dst.chain_id = e.dst_chain_id
WHERE i_src.chain_id IS NULL OR i_dst.chain_id IS NULL
ORDER BY e.transfers_count DESC;
```

**Expected result:** on a bridge with any unconfigured chains it talks to
(the norm for Avalanche), this should return rows — an empty result here on
a config with known unindexed chains for a bridge would suggest the
opt-in retention is not actually running, not that everything is fine.

**Normal vs. worth a second look:**
- `decimals IS NULL` — normal. The amount came from only the one observed
  side, so token metadata (decimals) may not have been fetched yet, or ever,
  for the unindexed side.
- `amount_side = 'destination'` (or `'source'`) — normal; it simply
  identifies which side's amount is backing `cumulative_amount`, since only
  one side is observable.
- Same reminder as D applies: re-check against the real `bridges.json`, not
  `bridge_contracts`, if a chain you know is indexed shows up here as
  `src_unindexed`/`dst_unindexed = true` on a database close to a fresh
  restart, or if a chain you removed from config months ago still shows as
  indexed (the permanent-staleness direction of the proxy caveat).

### F. Incoming ICTT reconstruction is landing

**What it checks:** transfers with both token addresses known whose parent
message has no `src_tx_hash`. Without the ICM-payload reconstruction path,
an incoming ICTT transfer observed only from the destination side cannot
have both token addresses populated at all — this row shape only exists
because of reconstruction (see ADR-004 References, and
`.memory-bank/gotchas.md`, "Message Finality is Complex").

**Why it matters:** direct evidence the reconstruction-from-ICM-payload path
is producing rows, as opposed to only having passed tests.

**Precondition — this query can only ever return rows if the bridge accepts
the message in the first place.** Incoming ICTT reconstruction runs on an
unknown-source message, and an unknown-source message only reaches
consolidation at all when the bridge has `process_unknown_chains: true`
(see `.memory-bank/gotchas.md`, "Events Filtered for Unconfigured Chains").
With the default `process_unknown_chains: false`, one-known/one-unknown
endpoint pairs are filtered out before reconstruction ever runs, so an empty
result on such a bridge says nothing about whether reconstruction works — it
is the expected result regardless. If the bridge also sets `home_chain_id`,
that filter must independently admit the message too (at least one endpoint
must equal `home_chain_id`) — a message that clears `process_unknown_chains`
but is narrowed out by `home_chain_id` is filtered the same way, before
reconstruction. Confirm both settings for the bridge in `bridges.json` before
treating an empty result here as informative one way or the other.

```sql
SELECT t.bridge_id, t.token_src_chain_id, t.token_dst_chain_id, m.status,
       t.src_amount, t.dst_amount, t.sender_address IS NULL AS no_sender
FROM crosschain_transfers t
JOIN crosschain_messages m ON m.id = t.message_id AND m.bridge_id = t.bridge_id
WHERE m.src_tx_hash IS NULL
  AND t.token_src_address IS NOT NULL AND t.token_dst_address IS NOT NULL
ORDER BY t.id DESC LIMIT 50;
```

**Expected result:** on any Avalanche bridge configuration where
`reconstruct_incoming_ictt_transfers` is enabled (the default) and any
destination chain is indexed while its counterpart source chain is not, you
should see rows here, growing over time as new incoming ICTT transfers
arrive. `no_sender = true` is possible and not itself concerning — the
sender address is not always recoverable purely from the destination-side
event.

**What an empty result means:** check, in this order, before treating it as
a defect — (1) `process_unknown_chains` is `false` for the bridge, the
default, in which case an empty result is expected regardless of anything
else (see the precondition above); (2) `home_chain_id` is set and does not
admit the relevant messages; (3) this bridge has no reachable scenario for
the reconstruction path yet (no destination-observed, source-unindexed ICTT
traffic has occurred); or (4) the per-bridge kill switch
(`reconstruct_incoming_ictt_transfers` in `bridges.json`) is off for it. Not
itself alarming — check whether the scenario should be reachable given your
bridge topology before treating this as a defect.

**If it flips off unexpectedly:** the reconstruction path can be disabled
per bridge via `reconstruct_incoming_ictt_transfers: false` in
`bridges.json` (default `true`). With it off, no new reconstructed row is
written and the underlying data is unrecoverable short of a reindex — but
the finality behavior in query G still applies regardless of this flag.

### G. `pending_messages` backlog trend (finality-leak canary)

**What it checks:** the current size and age of the `pending_messages`
cold-storage backlog, per bridge. Note this table's actual shape: keyed by
`(message_id, bridge_id)` — **it has no `id` column** — with `payload` and a
nullable `created_at`.

**Why it matters:** incoming ICTT transfers from an unindexed source chain,
and multi-hop first legs, are the two shapes that can legitimately sit here
indefinitely by design (see `.memory-bank/gotchas.md`, "Message Finality is
Complex" and "`pending_messages` Retention for Unconfigured Counterparts Is
Load-Bearing, Not a Leak"). Before the ICM-payload finality classification
existed, both shapes instead got stuck in `Partial` finality forever with no
way out, accumulating here unboundedly. This query does not prove the fix
in isolation — it's a trend to watch, not a one-shot canary.

```sql
SELECT bridge_id, count(*), min(created_at) AS oldest FROM pending_messages GROUP BY 1;
```

**Expected result / how to read it:** watch this over time, not as a single
snapshot. `oldest` receding indefinitely (getting older and older without
bound, on a bridge that's actively indexing) is the symptom of an
unbounded-accumulation defect. In healthy operation, entries still enter and
exit as ordinary message lifecycle events (hot buffer graduating to cold on
TTL, and back on a fresh event), and the backlog does not grow without
bound on its own — a shrinking-then-flat trend, or a steady plateau tied to
genuinely permanent one-sided messages (see the load-bearing-retention
gotcha above), is normal. A backlog that keeps growing at a steady rate on a
bridge with no such permanent shapes means a leak is present.

**One caveat on rollout:** this table does **not** auto-drain on deploy or
config change. Each existing cold entry only re-enters processing via a
**new** blockchain event for that same message key — there is no bulk
reload of `pending_messages` at startup. So immediately after any change
meant to reduce this backlog, expect the *existing* accumulated rows to
still be sitting here; what to actually watch is whether it keeps *growing*
the way it used to.

### H. Unclassified transfers (`asset_linkage IS NULL`) — stuck-row query

**What it checks:** transfers whose indexer has never stated `asset_linkage`,
grouped by bridge. Served by `crosschain_transfers_unclassified_idx`
(a partial index on `id` `WHERE asset_linkage IS NULL`), so this is cheap to
run on a large table even though it's a full scan of a normally-tiny set.

**Why it matters** ([ADR-011](../adr/011-cross-asset-edges-and-per-transfer-linkage.md)):
unlike `identity_incomplete` / `awaiting_confirmation`, a `linkage_unknown`
deferral does not self-heal as more chain data arrives — it can only be
resolved by the indexer actually stating a linkage on a later flush (via the
write-once `COALESCE`), or by manual intervention. In healthy operation this
should be **zero rows**, always: every shipped indexer states its linkage on
every transfer it builds (`interchain_indexer_entity::new_transfer`), and the
`flush_to_final_storage` chokepoint counts + warns (and `debug_assert!`s in
debug builds) on any omission. A nonzero, non-transient result here means
either a bug in an indexer's transfer constructor, or a genuinely new indexer
that has not been updated to declare linkage yet.

```sql
SELECT t.bridge_id, b.type, count(*) AS unclassified,
       min(t.id) AS example, min(t.created_at) AS oldest
FROM crosschain_transfers t JOIN bridges b ON b.id = t.bridge_id
WHERE t.asset_linkage IS NULL
GROUP BY 1, 2 ORDER BY unclassified DESC;
```

**Expected result:** zero rows.

**Recovery procedure, if this returns rows:**

1. Identify why the indexer for the affected `bridge_id` left `asset_linkage`
   unset — check `STATS_TRANSFER_ASSET_LINKAGE_UNSET_TOTAL` and the
   `tracing::warn!` it's paired with for the specific `message_id`s, and fix
   the transfer constructor to start from `new_transfer(linkage)`.
2. Once the fix is deployed and the indexer re-flushes the affected
   transfers (the write-once `COALESCE` fills `NULL -> value` on that later
   flush), restart with
   `INTERCHAIN_INDEXER__STATS__BACKFILL_ON_START=true`.
3. **Do not** reset `stats_processed` and do **not** run a full rebuild. A
   deferred (`linkage_unknown`) row was never counted, so its marker is
   already `0` — the backfill candidate query selects exactly
   `stats_processed = 0` rows with a per-run cursor
   (`database.rs`'s `backfill_stats_projection_round`, transfer-phase
   query), which will pick these rows up on its own once their
   `asset_linkage` is no longer `NULL`. Resetting markers or rebuilding
   would double-count rows this bridge already had counted correctly before
   the bug.

---

## Quick reference

| Query | Kind | Expected result | If it deviates |
| --- | --- | --- | --- |
| C — hard invariants | canary | `0, 0, 0, 0, 0` | stop; bug in shared projection/merge code, not a data issue |
| A — split-asset detector | canary | zero rows | run B for each returned pair before concluding anything |
| B — refusal legitimacy | diagnostic | row returned = correct refusal; empty = merge should have happened | empty result is a real defect — file it |
| D — deferred-transfer reasons | diagnostic | mostly expected buckets; two "watch" buckets | growth in `one-sided, peer unindexed` or unbounded `COMPLETE AND CONFIRMED` is a bug signal |
| E — unindexed-chain edges | diagnostic | rows present wherever a bridge has unconfigured chains | unexpectedly empty means opt-in retention isn't running; cross-check against real config, not `bridge_contracts` |
| F — incoming ICTT reconstruction | diagnostic | rows present where reachable, but only if `process_unknown_chains: true` and `home_chain_id` admits it | check `process_unknown_chains`/`home_chain_id` first, then the scenario/kill switch, before worrying |
| G — pending_messages trend | diagnostic | `oldest` stops receding indefinitely | still climbing at the old rate → leak persists |
| H — unclassified transfers (`asset_linkage IS NULL`) | canary | zero rows | fix the offending indexer's transfer constructor, then `BACKFILL_ON_START=true` — no marker reset, no rebuild |
