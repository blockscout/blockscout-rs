Interchain Indexer Service
===

**Interchain Indexer** — a standalone Rust microservice designed to index and aggregate cross-chain interactions across multiple networks.

It extends the Blockscout ecosystem with the ability to process and unify data from bridges, native interop protocols, and other interchain mechanisms.

Traditional Blockscout instances are designed for single-network indexing.  
However, as cross-chain ecosystems evolve, monitoring interactions between multiple chains becomes essential.

`interchain-indexer` provides an independent service that:
- Collects and indexes cross-chain transaction data directly from multiple networks.
- Supports various bridge types and native interop mechanisms via pluggable workers.
- Maintains its own database schema optimized for interchain data representation.
- Can operate without relying on single-chain Blockscout instances.
- Provides a set of endpoints for querying interchain messages and transfers.

## Common Design Principles

1. The service works across multiple networks (a cluster) and can index several bridges.
    - **Maximal variant:** multiple networks with heterogeneous cross-chain mechanisms (e.g., Optimism Superchain with native interop and lock/mint bridges).
    - **Base variant:** two networks connected by one bridge (e.g., Gnosis ↔ Ethereum via OmniBridge).
    - **Minimal variant:** indexing L1 → L2 deposits within a single L2.
2. The service must operate **independently** from Blockscout instances. Integration should be optional and not a dependency.
3. It should focus on **cross-chain interactions**, primarily token transfers, while retaining other metadata for potential future use and schema migrations.
4. Configuration (networks, bridges, contracts) should be stored as JSON in a local database, not as environment variables.
5. Each cross-chain mechanism's indexing logic should be implemented as a **plugin**, not a config parameter, to ensure scalability.
6. Planned workers:
    - **CrosschainIndexer** indexes a single bridge and optionally combines the BridgeContractIndexer and MessageCollector entities.
    - **BridgeContractIndexer** collects raw on-chain events for each bridge contract and stores them in `bridge_txs`.
    - **MessageCollector** processes raw events into structured cross-chain messages and transfers.
    - **TokenFetcher** fetches metadata for newly encountered tokens.
    - **Renderer** serves processed data to users and external consumers.

## Envs

### Main Service Settings

[anchor]: <> (anchors.envs.start.service)

| Variable                                                                | Req&#x200B;uir&#x200B;ed | Description                                                  | Default value |
| ----------------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------ | ------------- |
| `INTERCHAIN_INDEXER__BRIDGES_CONFIG`                                    | true                     | e.g. `config/avalanche/bridges.json`                         |               |
| `INTERCHAIN_INDEXER__CHAINS_CONFIG`                                     | true                     | e.g. `config/avalanche/chains.json`                          |               |
| `INTERCHAIN_INDEXER__DATABASE__CONNECT__URL`                            | true                     | e.g. `postgres://postgres:postgres@database:5433/blockscout` |               |
| `INTERCHAIN_INDEXER__DATABASE__CREATE_DATABASE`                         |                          | Create database on service startup                           | `false`       |
| `INTERCHAIN_INDEXER__DATABASE__RUN_MIGRATIONS`                          |                          | Run DB migrations on startup                                 | `false`       |
| `INTERCHAIN_INDEXER__API__DEFAULT_PAGE_SIZE`                            |                          | Default page size for paginated endpoints (`/api/v1/interchain/messages` and `/api/v1/interchain/transfers`) | `50`          |
| `INTERCHAIN_INDEXER__API__MAX_PAGE_SIZE`                                |                          | Maximum supported page size for paginated endpoints (configured via `page_size` query parameter) | `100`         |
| `INTERCHAIN_INDEXER__API__USE_PAGINATION_TOKEN`                         |                          | If true, wrap all raw pagination parameters into the single Base64 string | `true`        |
| `INTERCHAIN_INDEXER__TOKEN_INFO__BLOCKSCOUT_TOKEN_INFO__IGNORE_CHAINS`  |                          | The list of chain IDs to be ignored by token info service. Comma-separated list of identifiers without spaces (e.g. `42,1000`)                                                             | ``            |
| `INTERCHAIN_INDEXER__TOKEN_INFO__BLOCKSCOUT_TOKEN_INFO__RETRY_INTERVAL` |                          | If the token icon is not found in the external token info service do not retry fetching it during this interval. Unit: `seconds` | `3600`        |
| `INTERCHAIN_INDEXER__TOKEN_INFO__BLOCKSCOUT_TOKEN_INFO__URL`            |                          | External Blockscout token info service. E.g. `https://contracts-info-test.k8s-dev.blockscout.com` | `null`        |
| `INTERCHAIN_INDEXER__TOKEN_INFO__ONCHAIN_RETRY_INTERVAL`                |                          | If the on-chain request for the token info was unsuccessful, do not retry fetching it during this interval. Unit: `seconds` | `10`          |
| `INTERCHAIN_INDEXER__CHAIN_INFO__COOLDOWN_INTERVAL`                     |                          | If the chain name is unknown, do not retry DB query during this interval. Unit: `seconds` | `60`          |

[anchor]: <> (anchors.envs.end.service)

### Avalanche Indexer Settings

[anchor]: <> (anchors.envs.start.avalanche)

| Variable                                                        | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --------------------------------------------------------------- | ------------------------ | ----------- | ------------- |
| `INTERCHAIN_INDEXER__AVALANCHE_INDEXER__BATCH_SIZE`             |                          | Number of contract events to be pulled at once. | `1000`        |
| `INTERCHAIN_INDEXER__AVALANCHE_INDEXER__PULL_INTERVAL_MS`       |                          | Duration between pulling contract events. Unit: `milliseconds` | `10000`       |
| `INTERCHAIN_INDEXER__AVALANCHE_INDEXER__PROCESS_UNKNOWN_CHAINS` |                          | Enable messages/transfers processing from/to non-indexing chains. | `false`       |

[anchor]: <> (anchors.envs.end.avalanche)

### Metrics Settings (Prometheus-compatible)

[anchor]: <> (anchors.envs.start.metrics)

| Variable                               | Req&#x200B;uir&#x200B;ed | Description | Default value  |
| -------------------------------------- | ------------------------ | ----------- | -------------- |
| `INTERCHAIN_INDEXER__METRICS__ENABLED` |                          | Enable the metrics server | `false`        |
| `INTERCHAIN_INDEXER__METRICS__ADDR`    |                          | Address for the metrics listener | `0.0.0.0:6060` |
| `INTERCHAIN_INDEXER__METRICS__ROUTE`   |                          | HTTP path for metrics scraping | `/metrics`     |

[anchor]: <> (anchors.envs.end.metrics)

Expose the metrics port (default `6060`) when running in Docker (see docker-compose.yml) and scrape `{addr}{route}`.

### Auxiliary Settings

<details><summary>Server settings</summary>
<p>

[anchor]: <> (anchors.envs.start.server)

| Variable                                                           | Req&#x200B;uir&#x200B;ed | Description | Default value                            |
| ------------------------------------------------------------------ | ------------------------ | ----------- | ---------------------------------------- |
| `INTERCHAIN_INDEXER__SERVER__GRPC__ADDR`                           |                          |             | `0.0.0.0:8051`                           |
| `INTERCHAIN_INDEXER__SERVER__GRPC__ENABLED`                        |                          |             | `false`                                  |
| `INTERCHAIN_INDEXER__SERVER__HTTP__ADDR`                           |                          |             | `0.0.0.0:8050`                           |
| `INTERCHAIN_INDEXER__SERVER__HTTP__BASE_PATH`                      |                          |             | `null`                                   |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__ALLOWED_CREDENTIALS`      |                          |             | `true`                                   |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__ALLOWED_METHODS`          |                          |             | `PUT, GET, POST, OPTIONS, DELETE, PATCH` |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__ALLOWED_ORIGIN`           |                          |             | ``                                       |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__BLOCK_ON_ORIGIN_MISMATCH` |                          |             | `false`                                  |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__ENABLED`                  |                          |             | `false`                                  |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__MAX_AGE`                  |                          |             | `3600`                                   |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__SEND_WILDCARD`            |                          |             | `false`                                  |
| `INTERCHAIN_INDEXER__SERVER__HTTP__ENABLED`                        |                          |             | `true`                                   |
| `INTERCHAIN_INDEXER__SERVER__HTTP__MAX_BODY_SIZE`                  |                          |             | `2097152`                                |

[anchor]: <> (anchors.envs.end.server)

</p>
</details>

<details><summary>Tracing settings</summary>
<p>

[anchor]: <> (anchors.envs.start.tracing)

| Variable                                     | Req&#x200B;uir&#x200B;ed | Description | Default value    |
| -------------------------------------------- | ------------------------ | ----------- | ---------------- |
| `INTERCHAIN_INDEXER__JAEGER__AGENT_ENDPOINT` |                          |             | `127.0.0.1:6831` |
| `INTERCHAIN_INDEXER__JAEGER__ENABLED`        |                          |             | `false`          |
| `INTERCHAIN_INDEXER__TRACING__ENABLED`       |                          |             | `true`           |
| `INTERCHAIN_INDEXER__TRACING__FORMAT`        |                          |             | `default`        |

[anchor]: <> (anchors.envs.end.tracing)

</p>
</details>

## Dev

+ Install [just](https://github.com/casey/just) cli. Just is like make but better.
+ Install [dotenv-cli](https://www.npmjs.com/package/dotenv-cli)
+ Execute `just` to see available dev commands

```bash
just
```
+ Start dev postgres service by just typing

```bash
just start-postgres
```

+ For ORM codegen and migrations install [sea-orm-cli](https://www.sea-ql.org/SeaORM/docs/generate-entity/sea-orm-cli/)


+ Write initial migration inside `interchain-indexer-logic/migration/src/m20220101_000001_create_table`.
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
or run with ENVs from .env current
    ```
    just run-dev
    ```

## Troubleshooting

1. Invalid tonic version

```
`Router` and `Router` have similar names, but are actually distinct types
```

To fix this error you need to change tonic version of `tonic` in `blockscout-service-launcher` to `0.8`

For now you can only change in `Cargo.lock`