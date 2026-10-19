# Block Range Rescan

Read this when you need the running indexer to scan block ranges again through
`RescanBlockRanges` of the operator write API: what the method can and cannot
fix, what must be true before the call, how to run it and watch it drain, and
how to read its errors. Keys and the shared request format are in
[write-api.md](write-api.md); the design is in
[ADR-016](../adr/016-operator-write-api-v1.md), and the ledger it writes into is
in [ADR-005](../adr/005-failed-range-ledger-and-checkpoint-independence.md).

## Purpose And Contract

The method puts block ranges of one or more `(bridge_id, chain_id)` pairs into
the failed-range ledger (`indexer_failures`). The retry pass of the indexer that
runs the bridge reads that table every `failure_retry.scan_interval` (60 s by
default), scans chunks of at most `batch_size` blocks, processes them through
the normal path (buffer, consolidate, merge-upsert) and deletes the processed
ranges from the table.

**The contract is "scan these blocks again", not "repair the data".** A replay
deletes no rows, sets no value back to `NULL` and does not recount statistics
that are already counted. The class of a problem decides whether a rescan alone
fixes it:

| Class | What |
|---|---|
| (i) Fixed by rescan alone | missing messages / transfers / confirmations (dropped, undecoded, filtered events, a failed handler); a NULL that should be a value; a wrong non-NULL value in a prefer-incoming column when the window yields the right entry shape; destination-owned columns (AMB and Avalanche unknown-source: destination range is enough; Avalanche with a configured source: request both chains); config-scope backfills (new contract or event) |
| (ii) Only after resetting stored rows + stats rebuild | a value that should become NULL; write-once columns (`native_id`, `asset_linkage`); `init_timestamp` too early / `last_update_timestamp` too late; a terminal status where the truth is non-terminal; xDai destination-owned and transfer columns; spurious rows or wrong message keys; corrupted `pending_messages`; any change to an already-counted row (stats) |
| (iii) Not fixable by rescan | token metadata; history below the scan floor or no longer served by RPC; reorged-away data; xDai's canonical-execution choice; anything the replay cannot find again |

## What A Request May Contain

- At most 100 ranges, and at most 2,000,000 blocks in total after overlapping
  and adjacent ranges of a pair are merged. Larger backfills go in consecutive
  requests, each one drained before the next.
- Every pair is a configured, enabled indexing target of this process, and
  `from_block` is not below the pair's scan floor.
- The bridge's indexer must replay failed ranges: its type is supported and
  `failure_retry.enabled = true`.
- Every range ends at or below the last scanned block (`realtime_cursor - 1`).
  The check reads the stored `realtime_cursor`. It moves only when a scanned
  block touched the message buffer, so on a quiet bridge it can lag behind the
  chain head, and a request for the newest blocks is refused until the next
  event persists the cursor.
- No range touches the unscanned catch-up interval (the blocks catch-up has not
  reached yet): the running catch-up processes them anyway.
- No range overlaps or is adjacent to an open row of `indexer_failures` of its
  pair. Such a request is refused until the open range has drained.

The process that accepts the request needs the same database and the same bridge
and indexer configuration as the process that runs the bridge. The ranges are
consumed by the process that runs the bridge: its retry tick reads the table. A
process with every bridge disabled rejects every pair as not a target.

## Preconditions

- **The release with both retry-tick fixes is fully rolled out, with no old pod
  left running.** Without the first fix (`note_open`) a request is replayed
  forever; without the second (`resolved_since_snapshot`) a new range can be
  scanned one block at a time. Do not use the method before then.
- `failure_retry.enabled = true` for the bridge's indexer (the default).
- One process runs each bridge (see the gotcha "The Failure Ledger's Healthy Path
  Is DB-Free Only Because One Process Owns A Bridge").

## One Request At A Time

Send one request, wait until it has drained (see the procedure), then send the
next. The method does not lock the rows of a pair: two requests for the same pair
at the same time both pass the checks and write overlapping rows. No data is
lost, but `failed_blocks` is over-reported until the rows are resolved.

## Procedure

1. **Dry run.** Send the request with `"dry_run": true`. Nothing is written and
   no audit row is created. Read `scheduled_ranges` (the normalized request) and
   `estimates` (`estimated_drain_seconds` per bridge).
2. **Class (ii) problems only: reset the stored rows first.** This is destructive:
   do it carefully, in a maintenance window, with a backup. Delete the affected
   `crosschain_messages` (the `crosschain_transfers` of a message go with it by
   cascade), the related `pending_messages` and the anomaly rows.
3. **Request every network of the bridge for the same window** in one request, so
   that both halves of a message meet in one buffer write. A one-sided window
   leaves permanent `pending_messages`.
4. **Send the request** without `dry_run`. The response carries `audit_id`.
5. **Watch it drain.**
   - `GET /api/v1/status/indexing?bridge_id=<b>`: `failed_blocks` grows to about
     `requested_blocks` and falls to 0; `catchup_complete` is `false` meanwhile.
   - The queued rows, selected by the ranges of your request (replace `42` with
     the `audit_id` of the response):

     ```sql
     SELECT f.bridge_id, f.chain_id, f.from_block, f.to_block, f.attempts, f.updated_at
     FROM indexer_failures f
     WHERE EXISTS (
       SELECT 1
       FROM write_api_audit_log a,
            jsonb_array_elements(a.result->'scheduled_ranges') AS r
       WHERE a.id = 42
         AND f.bridge_id = (r->>'bridge_id')::int
         AND f.chain_id = (r->>'chain_id')::bigint
         AND f.from_block <= (r->>'to_block')::bigint
         AND f.to_block >= (r->>'from_block')::bigint)
     ORDER BY f.bridge_id, f.chain_id, f.from_block;
     ```

     Do not select the rows by `reason`. A queued row starts with
     `write-api: <actor>: <reason>`, but a replay chunk that fails again
     overwrites `reason` with the provider's error text, and so does a later
     merge with a neighbouring failure. A row that is still open can therefore
     stop looking like yours. The query matches by pair and block overlap
     instead, so it also returns a row that was merged into a wider one. For a
     dry run or a request without an audit row, put the ranges into the
     condition by hand.
6. **Wait for `failed_blocks == 0`.** This is the completion signal. The query
   above returns no rows and no new `scanning RETRY logs` entry for the range
   appears in the log.
7. **If already-counted rows changed** (class (ii)), rebuild the statistics as
   described in the README section "Stats projection maintenance rebuilds".
8. **Check the data** that motivated the request.

## Hole-Age Alert

A rescan makes `interchain_indexer_oldest_open_hole_age_seconds` of the bridge
grow while the ranges drain. If the alert on it fires for that reason, it is
noise; if it is silenced carelessly, a real hole hides behind it.

1. Take `estimated_drain_seconds` of the bridge from the dry run.
2. Find the threshold of the hole-age alert in the alerting configuration
   (`<placeholder: link to the alert rule>`; the rule is not in this repository).
3. If the estimate is at or above the threshold, silence the series for this
   `bridge_id` for the estimate plus a margin, and tell the on-call engineers.
4. The silence also hides real holes of this bridge for its whole length. Remove
   it as soon as `failed_blocks` reaches 0.

Expect `failed_blocks > 0` and `catchup_complete = false` for the same time.

## Example

Int64 and uint64 fields are JSON strings; `bridge_id` is a number.

```sh
curl -sS -X POST "$BASE/api/v1/admin/indexing:rescanBlockRanges" \
  -H 'content-type: application/json' -H "x-api-key: $KEY" \
  -d '{"ranges":[{"bridge_id":1,"chain_id":"1","from_block":"21000000","to_block":"21001000"},
                 {"bridge_id":1,"chain_id":"100","from_block":"39000000","to_block":"39005000"}],
       "reason":"TICKET-9","dry_run":true}'
```

The response of an applied request has the same shape with `audit_id` set; a dry
run returns `"audit_id": null`.

## Errors

The error body is `{"code": <gRPC code>, "message": "..."}`. Every violation of a
phase is returned in one message, separated by `; `.

- **400, code 3 (`INVALID_ARGUMENT`)**: the shape of the request, a limit, a pair
  that is not a configured target, a range below the scan floor, or an invalid
  `reason`. A violation of one element names it as
  `ranges[<i>] (bridge <b>, chain <c>, <from>..<to>)` with its position in the
  request; a limit names the limit and the actual value.
- **400, code 9 (`FAILED_PRECONDITION`)**: the request is well formed but the
  current state refuses it. The message names the normalized range
  (`bridge <b>, chain <c>, <from>..<to>`), not a position in the request, and one
  of:
  - replay is unavailable for the bridge (no indexer for its type, or replay
    disabled);
  - no checkpoint yet;
  - above the last scanned block;
  - inside the unscanned catch-up interval;
  - overlap or adjacency with an open failed range, named as
    `chain <c> <from>..<to>`. Wait until it has drained and send the request
    again.
- **401**: no valid key; the same for a dry run.

## Caveats

- `scheduled_ranges` (and `result.scheduled_ranges` in the audit row) are the
  normalized ranges of the request, not the rows of `indexer_failures`. If the
  indexer records an adjacent failure and its write commits first, the request's
  write merges with it and the stored row is wider. If the two writes are truly
  simultaneous, two adjacent rows can be left instead. No block is lost either
  way, and both rows are retried.
- `reason` of a stored row is a reference only. A failing replay chunk or a later
  merge overwrites it, so do not use it to find the rows of a request (see the
  query in the procedure).
- A request for a bridge whose indexer has stopped or escalated is accepted and
  waits as `failed_blocks` until the next successful start. The method does not
  check the state of the indexer.
- The estimate is approximate. The replay budget (`max_chunks_per_pass`) is
  shared with the real failed ranges of the bridge, and chunks that fail or get
  narrower take longer.
- Stopping the process between a range being resolved and the buffer flush loses
  part of that work. Send the request again after the process is back.
- A one-sided window leaves permanent `pending_messages`; on AMB it also leaves
  signatures in memory.
- Traps to know before reading the result:
  - a message can go from `completed` to `failed` when the window contains a
    failed execution without the successful retry;
  - xDai can report a false `multiple_executions`;
  - statistics can be counted twice on an AMB `messageId` collision.
