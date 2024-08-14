DA Indexer Service
===

The DA Indexer service collects blobs from different DA solutions (currently only Celestia and EigenDA) and provides a convenient API for fetching blob data. In addition to indexing blobs, this service can be configured to fetch L2 batch metadata corresponding to a specific blob (currently only available for Celestia).

## Celestia
The Celestia indexer runs on top of the [Celestia light node](https://docs.celestia.org/nodes/light-node). It is worth noting that the indexer collects only blobs and some block metadata, it does not collect full blocks, transactions, etc.

## EigenDA
The EigenDA indexer runs on top of the EigenDA disperser. It is worth mentioning that the disperser does not store blobs older than two weeks, so these blobs will be unavailable.

## Env

### General
| Variable                                                | Description                                            | Default value                    |
|---------------------------------------------------------|--------------------------------------------------------|----------------------------------|
| DA_INDEXER__DATABASE__CONNECT__URL                      | Postgres URL to stats db                               | ''                               |
| DA_INDEXER__DATABASE__CREATE_DATABASE                   | Boolean. Creates database on start                     | false                            |
| DA_INDEXER__DATABASE__RUN_MIGRATIONS                    | Boolean. Runs migrations on start                      | false                            |
| DA_INDEXER__INDEXER__CONCURRENCY                        | Number of jobs processed concurrently                  |                                  |
| DA_INDEXER__INDEXER__RESTART_DELAY                      | The delay between attempts to restart the indexer      | 60 seconds                       |
| DA_INDEXER__INDEXER__POLLING_INTERVAL                   | The delay between polling for new jobs from the node   | 12 seconds                       |
| DA_INDEXER__INDEXER__RETRY_INTERVAL                     | The delay between attempts to reprocess failed jobs    | 180 seconds                      |
| DA_INDEXER__INDEXER__CATCHUP_INTERVAL                   | The delay between attempts to process missing jobs     | 0 seconds                        |
| DA_INDEXER__DA__TYPE                                    | "Celestia" or "EigenDA"                                |                                  |
| DA_INDEXER__L2_ROUTER__ROUTES_PATH                      | Path to the routes config file                         |                                  |


### Celestia
| Variable                                                | Description                                            | Default value                    |
|---------------------------------------------------------|--------------------------------------------------------|----------------------------------|
| DA_INDEXER__INDEXER__DA__RPC__URL                       | Celestia light node RPC url                            |                                  |
| DA_INDEXER__INDEXER__DA__RPC__AUTH_TOKEN                | Celestia light node authorization token                | ''                               |
| DA_INDEXER__INDEXER__DA__START_HEIGHT                   | The height of the block to start with                  | The local head of the light node |

### EigenDA
| Variable                                                | Description                                            | Default value                    |
|---------------------------------------------------------|--------------------------------------------------------|----------------------------------|
| DA_INDEXER__INDEXER__DA__DISPERSER_URL                  | EigenDA disperser url                                  |                                  |
| DA_INDEXER__INDEXER__DA__EIGENDA_ADDRESS                | Address of the `EigenDAServiceManager`                 |                                  |
| DA_INDEXER__INDEXER__DA__EIGENDA_CREATION_BLOCK         | The `EigenDAServiceManager` creation block             |                                  |
| DA_INDEXER__INDEXER__DA__RPC__URL                       | Mainnet or Testnet `RPC_URL`                           |                                  |
| DA_INDEXER__INDEXER__DA__RPC__BATCH_SIZE                | Batch size to use in the `eth_getLogs` requests        |                                  |
| DA_INDEXER__INDEXER__DA__START_BLOCK                    | The number of the block to start with                  | The latest block number          |
| DA_INDEXER__INDEXER__DA__SAVE_BATCH_SIZE                | The number of blobs to save per db transaction         |                                  |
| DA_INDEXER__INDEXER__DA__PRUNING_BLOCK_THRESHOLD        | The threshold above which blobs might be unavailable   |                                  |

### L2 Batch Metadata
To fetch L2 batch metadata, the service must be aware of the L2s that use Celestia as a DA layer and the namespaces they utilize. This information is configured in a separate file, with its path specified in the `DA_INDEXER__L2_ROUTER__ROUTES_PATH` environment variable. Indexer and database configuration are optional if the `DA_INDEXER__L2_ROUTER__ROUTES_PATH` environment variable is set. An example of the routes config is shown below:
```toml
[routes.0x00000000000000000000000000000000000000000008e5f679bf7116cb]
l2_chain_type = "Optimism"
l2_chain_id = 123420111
l2_api_url = "https://opcelestia-raspberry.gelatoscout.com/"
l2_blockscout_url = "https://opcelestia-raspberry.gelatoscout.com/"

[routes.0x00000000000000000000000000000000000000ca1de12a1f4dbe943b6b]
l2_chain_type = "Arbitrum"
l2_chain_id = 123
l2_api_url = "http://localhost:3001"
l2_blockscout_url = "http://arbitrum.blockscout.com"
l1_chain_id = 456 # optional
```

## Dev

+ Install [just](https://github.com/casey/just) cli. Just is like make but better.
+ Execute `just` to see available dev commands

```bash
just
```
+ Start dev postgres service by just typing

```bash
just start-postgres
```

+ For ORM codegen and migrations install [sea-orm-cli](https://www.sea-ql.org/SeaORM/docs/generate-entity/sea-orm-cli/)


+ Write initial migration inside `da-indexer-logic/migration/src/m20220101_000001_create_table`.
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

## Troubleshooting

1. Invalid tonic version

```
`Router` and `Router` have similar names, but are actually distinct types
```

To fix this error you need to change tonic version of `tonic` in `blockscout-service-launcer` to `0.8`

For now you can only change in `Cargo.lock`
