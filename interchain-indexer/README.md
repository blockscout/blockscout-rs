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

Metrics (Prometheus-compatible):

| Variable | Description | Default |
| --- | --- | --- |
| `INTERCHAIN_INDEXER__METRICS__ENABLED` | Enable the metrics server | `false` |
| `INTERCHAIN_INDEXER__METRICS__ADDR` | Address for the metrics listener | `0.0.0.0:6060` |
| `INTERCHAIN_INDEXER__METRICS__ROUTE` | HTTP path for metrics scraping | `/metrics` |

Expose the metrics port (default `6060`) when running in Docker (see docker-compose.yml) and scrape `{addr}{route}`.

[anchor]: <> (anchors.envs.start)
[anchor]: <> (anchors.envs.end)

## Dev

+ Install [just](https://github.com/casey/just) cli. Just is like make but better.
+ Install [dotenv-cli](https://www.npmjs.com/package/dotenv-cli)
+ Execute `just` to see avaliable dev commands

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

To fix this error you need to change tonic version of `tonic` in `blockscout-service-launcer` to `0.8`

For now you can only change in `Cargo.lock`
