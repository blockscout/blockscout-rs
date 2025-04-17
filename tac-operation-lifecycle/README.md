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
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT__URL` | true | e.g. `postgres://postgres:postgres@database:5432/blockscout` | |
| `TAC_OPERATION_LIFECYCLE__INDEXER__CONCURRENCY` | true |  Number of concurrent operations the indexer can process  | number of logical CPU's |
| `TAC_OPERATION_LIFECYCLE__RPC__URL` | true | RPC endpoint e.g. `https://data.turin.tac.build/` | |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__ACQUIRE_TIMEOUT` | | e.g. `10` | `null` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__CONNECT_TIMEOUT` | | e.g. `10` | `null` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__IDLE_TIMEOUT` | | | `null` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__MAX_CONNECTIONS` | | e.g. `20` | `null` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__MAX_LIFETIME` | | | `null` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__MIN_CONNECTIONS` | | e.g. `10` | `null` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__SQLX_LOGGING` | | | `true` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CONNECT_OPTIONS__SQLX_LOGGING_LEVEL` | | | `debug` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__CREATE_DATABASE` | | e.g. `true` | `false` |
| `TAC_OPERATION_LIFECYCLE__DATABASE__RUN_MIGRATIONS` | | e.g. `true` | `false` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__CATCHUP_INTERVAL` | |  The catchup_interval defines the size of time windows used for processing historical data. Smaller intervals provide more granular processing but may increase the number of RPC calls. | `5` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__POLLING_INTERVAL` | | The polling_interval determines how frequently the indexer checks for new data. Setting it to 0 disables polling. | `0` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__RESTART_DELAY` | | | `60` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__RETRY_INTERVAL` | | The retry_interval is used as the base for exponential backoff when retrying failed operations. The actual retry delay will increase exponentially with each retry attempt.| `180` |
| `TAC_OPERATION_LIFECYCLE__INDEXER__START_TIMESTAMP` | | The start_timestamp allows you to specify a custom starting point for historical data indexing. Setting it to 0 means the indexer will  start from the earliest available data. All of the events before this epoch are essentially ignored. This could be used for partial sync | `0` |
| `TAC_OPERATION_LIFECYCLE__RPC__AUTH_TOKEN` | | | `null` |
| `TAC_OPERATION_LIFECYCLE__RPC__MAX_REQUEST_SIZE` | | | `104857600` |
| `TAC_OPERATION_LIFECYCLE__RPC__MAX_RESPONSE_SIZE` | | | `104857600` |
| `TAC_OPERATION_LIFECYCLE__RPC__REQUEST_PER_SECOND` | | | `100` |

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