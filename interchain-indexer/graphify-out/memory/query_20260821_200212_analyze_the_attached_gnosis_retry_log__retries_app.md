---
type: "query"
date: "2026-08-21T20:02:12.826817+00:00"
question: "Analyze the attached Gnosis retry log: retries appear to multiply failed blocks in a loop, and the service is still running on localhost:8050"
contributor: "graphify"
outcome: "useful"
source_nodes: ["RangeDriver::run / run_retry_tick (indexer/range_driver.rs)", "fetch_receipts_for_transactions()", "FailureLedger / indexer_failures", "retry_queue()"]
---

# Q: Analyze the attached Gnosis retry log: retries appear to multiply failed blocks in a loop, and the service is still running on localhost:8050

## Answer

Expanded from original query via graph vocab: [range, driver, retry, retried, failed, chunk, block, receipt, transaction, catchup, checkpoint, cursor]. Live diagnosis: retry re-records are set-union and do not duplicate block coverage; the rapid growth comes from new catchup batches. The Gnosis pool order is blockscout, drpc, gnosis_official. For all 22 transactions in a recorded historical failed range, blockscout and gnosis_official return receipts while drpc returns null for all 22. A null JSON-RPC result is treated as transport success, so no provider failover occurs. fetch_receipts_for_transactions fails fast before block attribution, and BatchError therefore records the entire 500-block batch. Concurrent retry load can exhaust the primary limiter and spill receipt calls to drpc, producing a feedback loop. Adjacent records merge without overlap but increment attempts and updated_at, which can push the merged interval to capped backoff. Live DB had zero overlaps, 635 rows, and 2,636,500 failed blocks at the final snapshot.

## Outcome

- Signal: useful

## Source Nodes

- RangeDriver::run / run_retry_tick (indexer/range_driver.rs)
- fetch_receipts_for_transactions()
- FailureLedger / indexer_failures
- retry_queue()