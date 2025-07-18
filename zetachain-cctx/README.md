Zetachain Cctx Service
===

## Indexer architecture
### The Indexer owns:
* settings – intervals, batch sizes, concurrency limits.
* database – a higher-level helper (ZetachainCctxDatabase) with domain-specific queries and inserts.
* client – RPC client that talks to the ZetaChain node.
It creates five independent asynchronous streams + one dedicated task.
All streams yield IndexerJobs that are funnelled into a single combined stream and processed with bounded concurrency.
Priority order (highest first)
1. Level-data-gap stream
* Checks watermark.kind == Realtime entries.
* If real-time fetching skipped some pages (node returned too many items in one go), fetches the missing range and marks the watermark Done / Failed.
2. Status-update stream
* Queries the DB for cctxs whose status is stale (query_cctxs_for_status_update).
* For each, schedules StatusUpdate job → fetch latest status via client.fetch_cctx → update row, refresh parent/child links.
3. Historical stream
* Grabs the next unlocked Historical watermark, locks it, pulls an older page of cctxs, inserts them, creates / updates the next watermark.
* Runs until chain genesis.
4. Failed-cctxs stream
* Re-queues cctxs previously marked Failed (exceeded retry threshold) for another status update attempt.

Separate thread (time-critical)

*  Realtime-fetch handler
    * Periodically calls client.list_cctxs(None, /* realtime = false */) to get “tip-of-chain” data.
    * Inserts new cctxs and creates/updates the Realtime watermark that covers the same page.

### Job types

* StatusUpdate(CctxShort, job_id)
    * Refresh single cctx status and update its tree relations (uses get_inbound_hash_to_cctx_data).
    * On error increments retries_number; on too many failures marks the cctx as Failed.
* LevelDataGap(watermark, job_id)
    * Fetch page starting from watermark.pointer, insert any missing cctxs, advance / fail / unlock the watermark depending on result.
* HistoricalDataFetch(watermark, job_id)
    * Same as above but for historical back-fill; cannot skip pages (must be strictly sequential).

### Concurrency & fault-tolerance
for_each_concurrent(Some(settings.concurrency)) – at most N jobs run in parallel.
Each job is idempotent and guarded by DB “locks”:
cctx rows carry processing_status (implicitly via retries_number + helper queries).
watermarks are locked with processing_status = Locked.
Retries: every failed job increments a counter and is re-queued until retry_threshold is hit, after which it is moved to Failed.
## Envs

[anchor]: <> (anchors.envs.start)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `ZETACHAIN_CCCTX__DATABASE__CONNECT__URL` | true | e.g. `postgres://postgres:postgres@database:5433/blockscout` | |
| `ZETACHAIN_CCCTX__INDEXER__ENABLED` | true | e.g. `true` | |
| `ZETACHAIN_CCCTX__INDEXER__FAILED_CCTXS_POLLING_INTERVAL` | true | e.g. `10000` | |
| `ZETACHAIN_CCCTX__INDEXER__HISTORICAL_BATCH_SIZE` | true | e.g. `1` | |
| `ZETACHAIN_CCCTX__INDEXER__POLLING_INTERVAL` | true | e.g. `2000` | |
| `ZETACHAIN_CCCTX__INDEXER__REALTIME_FETCH_BATCH_SIZE` | true | e.g. `10` | |
| `ZETACHAIN_CCCTX__INDEXER__REALTIME_THRESHOLD` | true | e.g. `10000` | |
| `ZETACHAIN_CCCTX__INDEXER__RETRY_THRESHOLD` | true | e.g. `10` | |
| `ZETACHAIN_CCCTX__INDEXER__STATUS_UPDATE_BATCH_SIZE` | true | e.g. `5` | |
| `ZETACHAIN_CCCTX__RPC__NUM_OF_RETRIES` | true | e.g. `30` | |
| `ZETACHAIN_CCCTX__RPC__REQUEST_PER_SECOND` | true | e.g. `10` | |
| `ZETACHAIN_CCCTX__RPC__RETRY_DELAY_MS` | true | e.g. `500` | |
| `ZETACHAIN_CCCTX__RPC__URL` | true | e.g. `https://zetachain-athens.g.allthatnode.com/archive/rest/{$YOUR_API_KEY}/zeta-chain/` | |
| `ZETACHAIN_CCCTX__DATABASE__CONNECT_OPTIONS__ACQUIRE_TIMEOUT` | | e.g. `10` | `null` |
| `ZETACHAIN_CCCTX__DATABASE__CONNECT_OPTIONS__CONNECT_LAZY` | | | `false` |
| `ZETACHAIN_CCCTX__DATABASE__CONNECT_OPTIONS__CONNECT_TIMEOUT` | | e.g. `10` | `null` |
| `ZETACHAIN_CCCTX__DATABASE__CONNECT_OPTIONS__IDLE_TIMEOUT` | | | `null` |
| `ZETACHAIN_CCCTX__DATABASE__CONNECT_OPTIONS__MAX_CONNECTIONS` | | e.g. `20` | `null` |
| `ZETACHAIN_CCCTX__DATABASE__CONNECT_OPTIONS__MAX_LIFETIME` | | | `null` |
| `ZETACHAIN_CCCTX__DATABASE__CONNECT_OPTIONS__MIN_CONNECTIONS` | | e.g. `10` | `null` |
| `ZETACHAIN_CCCTX__DATABASE__CONNECT_OPTIONS__SQLX_LOGGING` | | | `true` |
| `ZETACHAIN_CCCTX__DATABASE__CONNECT_OPTIONS__SQLX_LOGGING_LEVEL` | | | `debug` |
| `ZETACHAIN_CCCTX__DATABASE__CONNECT_OPTIONS__SQLX_SLOW_STATEMENTS_LOGGING_LEVEL` | | | `off` |
| `ZETACHAIN_CCCTX__DATABASE__CONNECT_OPTIONS__SQLX_SLOW_STATEMENTS_LOGGING_THRESHOLD` | | | `1` |
| `ZETACHAIN_CCCTX__DATABASE__CREATE_DATABASE` | | e.g. `true` | `false` |
| `ZETACHAIN_CCCTX__DATABASE__RUN_MIGRATIONS` | | e.g. `true` | `false` |

[anchor]: <> (anchors.envs.end)