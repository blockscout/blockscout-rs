Tac Operation Lifecycle Service
===

## Intro

[Ton Application Chain](https://docs.tac.build/) (TAC) is an EVM-compatible extension for the TON blockchain.
TAC transactions are ordered and executed under a separate consensus mechanism, with only the execution results being posted back on-chain — a pattern somewhat similar to how rollups operate.

Users interact with TAC either through dedicated dApps or via smart contracts on the TON chain. Regardless of the method, the user’s intention — along with all resulting artifacts — is encapsulated under a single concept called an “Operation.”

These Operations can involve different data flows and a variety of status types. This complexity can be confusing for inexperienced users and may hinder debugging for dApp developers.

This feature addresses that by indexing TAC Operations in a way that provides a clear, high-level overview of what occurred as a result of a user’s action — making it easier for both users and developers to understand at a glance.

## Improvements 

Indexer Logic Description
The TAC Operation Lifecycle Indexer follows a three-stage process:
1. Timeline Dissection:
* The indexer divides the timeline into fixed-size `intervals`
* It maintains a `watermark` that marks the latest processed timestamp
* The `watermark` advances as `intervals` are processed
* For historical data, it processes `intervals` from oldest to newest
For realtime data, it continuously creates new `intervals`
2. Interval Processing:
* For each interval, the indexer fetches a list of operations that occurred within that time window
* Operations are stored in the database with a `pending` status
* The interval is marked as `finalized` once operations are fetched
* If fetching fails, the interval is scheduled for retry with exponential backoff
3. Operation Processing:
* For each operation, the indexer fetches detailed stage information
* Operation stages track the lifecycle of the operation across different blockchains
* Once stages are fetched, the operation is marked as `finalized`
* If fetching fails, the operation is scheduled for retry with exponential backoff


```
+----------------------------------------------------------------------------------------+
|                                    TAC OPERATION LIFECYCLE INDEXER                      |
+----------------------------------------------------------------------------------------+
                                                                                          
                                                                                          
+-------------------+     +-------------------+     +-------------------+                  
|                   |     |                   |     |                   |                  
|  Timeline         |     |  Intervals        |     |  Operations       |                  
|  Dissection       |---->|  Processing       |---->|  Processing       |                  
|                   |     |                   |     |                   |                  
+-------------------+     +-------------------+     +-------------------+                  
        |                         |                         |                              
        |                         |                         |                              
        v                         v                         v                              
+-------------------+     +-------------------+     +-------------------+                  
|                   |     |                   |     |                   |                  
|  Watermark        |     |  Fetch            |     |  Fetch            |                  
|  Advancement      |     |  Operations       |     |  Operation        |                  
|                   |     |  for Interval     |     |  Stages           |                  
+-------------------+     +-------------------+     +-------------------+  
```

We persist and track latest saved interval (`watermark`) in the database and advance it alongside with creating new intervals.
Apart from latest interval we also track latest `operation` so that if we get a falsely empty response we would automatically request it again.
This PR follows similar practices  from `da_indexer` specifically the server launches multiple future streams:
* historic operation fetcher that selects `intervals` in ascending order from a configurable starting timestamp
* realtime operation fetcher that selects `intervals` in ascending order after the service has started
* failed intervals and operations fetcher resends failed requests with exponential backoff 

```                                                                                       
+----------------------------------------------------------------------------------------+
|                                    PRIORITIZED STREAMS                                  |
+----------------------------------------------------------------------------------------+
                                                                                          
                                                                                          
+-------------------+     +-------------------+     +-------------------+                  
|                   |     |                   |     |                   |                  
|  Realtime         |     |  Historical       |     |  Operations       |                  
|  Stream           |     |  Intervals        |     |  Stream           |                  
|                   |     |  Stream           |     |                   |                  
+-------------------+     +-------------------+     +-------------------+                  
        |                         |                         |                              
        |                         |                         |                              
        v                         v                         v                              
+----------------------------------------------------------------------------------------+
|                                    COMBINED STREAM                                      |
+----------------------------------------------------------------------------------------+
```

## Configuration Parameters

Parameters can be configured either using a `yaml`file or environment variables. See example in `tac-operation-lifecycle-server/config.yaml`

[anchor]: <> (anchors.envs.start.service)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `TAC_OPERATION_LIFECYCLE__INDEXER__CONCURRENCY` | | The number of jobs simultaneously fetched from the common job stream. | `14` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__CATCHUP_INTERVAL` | | The size of time windows (in seconds) used for processing historical data. Smaller intervals provide more granular processing but may increase the number of RPC calls. | `5` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__POLLING_INTERVAL` | | Determines how frequently the indexer checks for new (realtime) data. The value is provided in seconds. | `2` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__RETRY_INTERVAL` | | Determines how frequently the indexer will retry failed intervals and operations. The value is provided in seconds. | `120` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__START_TIMESTAMP` | | Specifies a custom starting point for historical data indexing. Setting it to `0` means the indexer will start from the earliest available data (this will significantly increase sync time). Events before this epoch are ignored. Useful for partial sync. | `0` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__FOREVER_PENDING_OPERATIONS_AGE_SEC` | | The operation is considered completed if it is older than this value (in seconds) but remains in the `PENDING` state. The value is hardcoded by the protocol and equals one week. | `604800` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__INTERVALS_QUERY_BATCH` | | The number of pending intervals simultaneously fetched from the database to be processed. Lower values will reduce database load. | `10` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__INTERVALS_RETRY_BATCH` | | The number of failed intervals simultaneously fetched from the database during the retry cycle. Lower values will reduce database load. | `10` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__INTERVALS_LOOP_DELAY_MS` | | Delay between interval fetches (from the database) to prevent a tight loop. The value is in milliseconds. | `100` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__OPERATIONS_QUERY_BATCH` | | The number of pending operations simultaneously fetched from the database to be processed. Lower values will reduce database load. | `10` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__OPERATIONS_RETRY_BATCH` | | The number of failed operations simultaneously fetched from the database during the retry cycle. Lower values will reduce database load. | `10` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__OPERATIONS_LOOP_DELAY_MS` | | Delay between operation fetches (from the database) to prevent a tight loop. The value is in milliseconds. | `200` |
| `TAC_OPERATION_LIFECYCLE__RPC__URL` | | TAC Staging Service RPC endpoint. | `https://data.turin.tac.build/` |
| `TAC_OPERATION_LIFECYCLE__RPC__AUTH_TOKEN` | | Currently not used. | `null` |
| `TAC_OPERATION_LIFECYCLE__RPC__REQUEST_PER_SECOND` | | The rate limit for requests per second. | `100` |
| `TAC_OPERATION_LIFECYCLE__RPC__NUM_OF_RETRIES` | | The number of retries for each request. A request is considered failed after this number of retries. | `10` |
| `TAC_OPERATION_LIFECYCLE__RPC__RETRY_DELAY_MS` | | The delay in milliseconds between retries. | `1000` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CREATE_DATABASE` | | Whether to create the database if it does not exist. | `false` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__RUN_MIGRATIONS` | | Whether to run database migrations on startup. | `false` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT__URL` | | The database connection URL (e.g., `postgres://postgres:postgres@database:5432/blockscout`). | None |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__ACQUIRE_TIMEOUT` | | The timeout (in seconds) for acquiring a database connection. | `null` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__CONNECT_TIMEOUT` | | The timeout (in seconds) for establishing a database connection. | `null` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__IDLE_TIMEOUT` | | The timeout (in seconds) for idle database connections. | `null` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__MAX_CONNECTIONS` | | The maximum number of database connections. | `null` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__MAX_LIFETIME` | | The maximum lifetime (in seconds) of a database connection. | `null` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__MIN_CONNECTIONS` | | The minimum number of database connections. | `null` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__SQLX_LOGGING` | | Whether to enable SQLx logging. | `true` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__SQLX_LOGGING_LEVEL` | | The logging level for SQLx. | `debug` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__CONNECT_LAZY` | | Whether to establish database connections lazily. | `false` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__SQLX_SLOW_STATEMENTS_LOGGING_LEVEL` | | The logging level for slow SQL statements. | `off` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__SQLX_SLOW_STATEMENTS_LOGGING_THRESHOLD` | | The threshold (in seconds) for logging slow SQL statements. | `1` |

[anchor]: <> (anchors.envs.end.service)

## Dev

+ Install [just](https://github.com/casey/just) cli. Just is like make but better.
+ Execute `just` to see avaliable dev commands

```bash
just
```
+ Start dev postgres service by just typing

```bash
just start-postgres
```

+ For ORM codegen and migrations install [sea-orm-cli](https://www.sea-ql.org/SeaORM/docs/generate-entity/sea-orm-cli/)


+ Write initial migration inside `tac-operation-lifecycle-logic/migration/src/m20220101_000001_create_table`.
+ If you want you can create another migration by just typing:

```bash
just new-migration <name>
```
+ Apply migration by just typing:

```bash
just migrate-up
```

+ Generate ORM codegen by just typing:

```bash
just generate-entities
```
+ Now you ready to start API server! Just run it:
```
just run
```