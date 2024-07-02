DA Indexer Service
===

The DA Indexer service collects blobs from different DA solutions (currently only Celestia and EigenDA) and provides a convenient API for fetching blob data.

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


### Celestia
| Variable                                                | Description                                            | Default value                    |
|---------------------------------------------------------|--------------------------------------------------------|----------------------------------|
| DA_INDEXER__INDEXER__DA__RPC__URL                       | Celestia light node RPC url                            |                                  |
| DA_INDEXER__INDEXER__DA__RPC__AUTH_TOKEN                | Celestia light node authorization token                | ''                               |
| DA_INDEXER__INDEXER__DA__START_HEIGHT                   | The height of the block to start with                  | The local head of the light node |

### EigenDA
| Variable                                                | Description                                            | Default value                    |
|---------------------------------------------------------|--------------------------------------------------------|----------------------------------|
| DA_INDEXER__INDEXER__DA__DISPERSER                      | EigenDA disperser url                                  |                                  |
| DA_INDEXER__INDEXER__DA__CONTRACT_ADDRESS               | Address of the `EigenDAServiceManager`                 |                                  |
| DA_INDEXER__INDEXER__DA__CONTRACT_CREATION_BLOCK        | The `EigenDAServiceManager` creation block             |                                  |
| DA_INDEXER__INDEXER__DA__RPC__URL                       | Mainnet or Testnet `RPC_URL`                           |                                  |
| DA_INDEXER__INDEXER__DA__RPC__BATCH_SIZE                | Batch size to use in the `eth_getLogs` requests        |                                  |
| DA_INDEXER__INDEXER__DA__START_HEIGHT                   | The number of the block to start with                  | The latest block number          |
| DA_INDEXER__INDEXER__DA__SAVE_BATCH_SIZE                | The number of blobs to save per db transaction         |                                  |
| DA_INDEXER__INDEXER__DA__PRUNING_BLOCK_THRESHOLD        | The threshold above which blobs might be unavailable   |                                  |


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
