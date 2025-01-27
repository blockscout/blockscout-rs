DA Indexer Service
===

The DA Indexer service collects blobs from different DA solutions (currently only Celestia and EigenDA) and provides a convenient API for fetching blob data. In addition to indexing blobs, this service can be configured to fetch L2 batch metadata corresponding to a specific blob (currently only available for Celestia).

This service supports three primary use cases:
* [Celestia Blob Indexer](#celestia-blob-indexer)
* [EigenDA Blob Indexer](#eigenda-blob-indexer)
* [L2 Batch Metadata](#l2-batch-metadata)

## Env

### General
| Variable                                                | Description                                            | Default value                    |
|---------------------------------------------------------|--------------------------------------------------------|----------------------------------|
| DA_INDEXER__DATABASE__CONNECT__URL                      | Postgres URL to db                                     | ''                               |
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

## Celestia Blob Indexer
The Celestia indexer runs on top of the [Celestia light node](https://docs.celestia.org/nodes/light-node). It is worth noting that the indexer collects only blobs and some block metadata, it does not collect full blocks, transactions, etc.

### Config example
```env
DA_INDEXER__DATABASE__CONNECT__URL=postgres://postgres:postgres@database:5432/blockscout
DA_INDEXER__DATABASE__CREATE_DATABASE="true"
DA_INDEXER__DATABASE__RUN_MIGRATIONS="true"

DA_INDEXER__INDEXER__DA__TYPE="Celestia"
DA_INDEXER__INDEXER__DA__RPC__URL="http://celestia-light-node:26658"
DA_INDEXER__INDEXER__CONCURRENCY=15
```

### API

**GET: `/api/v1/celestia/blob`**

Query Params:

- **height** (required) - The number of Celestia block that contains the requested Celestia blob
- **commitment** (required) - The commitment of Celestia blob in hex or base64
- **skipData** (optional) - If true, the response will contain only blob metadata without the blob itself

Response:
```json
{
   "height":"3610160",
   "namespace":"00000000000000000000000000000000000000115d4fedc8915bb3e875",
   "commitment":"y7dYpzDxhAiYZOvetTASeLO2ceiWJ8CYI5HeABddAlc=",
   "timestamp":"1734016259",
   "size":"305",
   "data":"G9IBAGRAT6spnZHRugDjkwAbcJ5g4r7yZDzDDRTW5gUA51AA7K5Bc1nYYbrobaLpDg9LWmxA1eJli3TQtF3wW87Ot9vC9wZQHWz9XDl5cQt+YqHkSPT/r5tBVNyjzS5cOvD23x9Jh8IkO81w2PyDfFEyUajff4nTvObifASr3rT92Qity9zjGBNDt7YjSQbq7aQflXLyUvjAk/ZpufxfSsLCn9f5ArcBBuZW2VOHoiyN6pwCCn+9fkM6BgBkmkhAIcmhvui8NQ2SvaAQXdm9SwLZ16HVQyIo1QK37qRwNiYAZJoom7hQUjhwAQXAYl8nGb+m63TDydVUMnwOQRZbS8fx5/qQiHd1T0b2yBN8n1kKxUQP/45Hzh4aoE7qbaiEo2iGk8aQSjy2prVvvq85bwA="
}
```

## EigenDA Blob Indexer
The EigenDA indexer runs on top of the EigenDA disperser. It is worth mentioning that the disperser does not store blobs older than two weeks, so these blobs will be unavailable.

### Config example
```env
DA_INDEXER__DATABASE__CONNECT__URL=postgres://postgres:postgres@database:5432/blockscout
DA_INDEXER__DATABASE__CREATE_DATABASE="true"
DA_INDEXER__DATABASE__RUN_MIGRATIONS="true"

DA_INDEXER__INDEXER__CONCURRENCY=5

DA_INDEXER__INDEXER__DA__TYPE="EigenDA"
DA_INDEXER__INDEXER__DA__DISPERSER_URL="https://disperser-holesky.eigenda.xyz:443"
DA_INDEXER__INDEXER__DA__EIGENDA_ADDRESS="0xD4A7E1Bd8015057293f0D0A557088c286942e84b"
DA_INDEXER__INDEXER__DA__EIGENDA_CREATION_BLOCK=1168412
DA_INDEXER__INDEXER__DA__SAVE_BATCH_SIZE=20
DA_INDEXER__INDEXER__DA__PRUNNING_BLOCK_THRESHOLD=1000

DA_INDEXER__INDEXER__DA__RPC__URL="https://ethereum-holesky-rpc.publicnode.com"
DA_INDEXER__INDEXER__DA__RPC__BATCH_SIZE=1000
```

### API
**GET: `/api/v1/eigenda/blob`**

Query Params:

- **batchHeaderHash** (required) - The hash of the batch header
- **blobIndex** (required) - The index of the blob in the batch
- **skipData** (optional) - If true, the response will contain only blob metadata without the blob itself

Response:
```json
{
    "batchHeaderHash": "49be2c7f2a26ef1574b78333fd5e7aa26f870cb7882d6676e084a7e396b5268e",
    "batchId": "120158",
    "blobIndex": 2,
    "l1ConfirmationBlock": "3116005",
    "l1ConfirmationTxHash": "0x69016622a1cefaf8b6fa378ff8e6e9368e51fa27571952a2cf3f26ad5cf8185f",
    "size": "262144",
    "data": "AJggbiH/w4XEv3+3RtGyM4cP0kQVzrB8P2/T+gxAUmQAXPOZT7QohOhfoEtw+wJ+qOQbH7Foj+jTXnmG08rbggAKUEzY0ctZ56ci5KQeSfZpPdeU6f7AejfPi14ZHh3LAD7bWnfEOv3kr2mFSy37YGpHZGomwkpuJxSf2ZaI/uoAWHh08qF+wnKJbdSCh0nKsxsLHRZVamtZ3GbKRlGM+wAz60WWy/LhkGHBVOeFSvw7n1CsDiCDw0yrXQxORxEYANfnsX2vhtxbVKoJioWl++OmVHfAhNvfe5rLX4GefNgAwQ9rtNK5B9wE/Fa5zP+6ruphMMjpMqi4L00THdlphwCq4agCDYgUm69vKPoPaowW73nS/lTCQ=="
}
```


## L2 Batch Metadata
This service can be used to fetch L2 batch metadata using the Celestia blob identifier. This service is designed for external use and is not required for indexing purposes.
To fetch L2 batch metadata, the service must be aware of the L2s that use Celestia as a DA layer and the namespaces they utilize. This information is configured in a separate file, with its path specified in the `DA_INDEXER__L2_ROUTER__ROUTES_PATH` environment variable. Indexer and database configuration are optional if the `DA_INDEXER__L2_ROUTER__ROUTES_PATH` environment variable is set. 

### Config example
```env
DA_INDEXER__L2_ROUTER__ROUTES_PATH=/app/celestia_routes.toml
```
```toml
# /app/celestia_routes.toml

[routes.0x00000000000000000000000000000000000000ca1de12a9905be97beaf]
l2_chain_type = "Arbitrum"
l2_chain_id = 123
l2_api_url = "http://localhost:4000"
l2_blockscout_url = "http://arbitrum.blockscout.com"

[routes.0x00000000000000000000000000000000000000000008e5f679bf7116cb]
l2_chain_type = "Optimism"
l2_chain_id = 123420111
l2_api_url = "https://opcelestia-raspberry.gelatoscout.com/"
l2_blockscout_url = "https://opcelestia-raspberry.gelatoscout.com/"
request_timeout = 30
request_retries = 1
```

### API
**GET: `/api/v1/celestia/l2BatchMetadata`**

Query Params:

- **height** (required) - The number of Celestia block that contains the requested Celestia blob
- **namespace** (required) - The namespace of Celestia blob in hex or base64
- **commitment** (required) - The commitment of Celestia blob in hex or base64

Response:
```json
{
    "l2ChainId": 123420111,
    "l2BatchId": "309555",
    "l2StartBlock": "10760977",
    "l2EndBlock": "10762344",
    "l2BatchTxCount": 1579,
    "l2BlockscoutUrl": "https://opcelestia-raspberry.gelatoscout.com/batches/309555",
    "l1TxHash": "0x704b1d4a9934e2123d2d431ff060f4739f478351a95d84e4775e55c08262f2f2",
    "l1TxTimestamp": "1724516808",
    "relatedBlobs": []
}
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
+ Now you are ready to start API server! Just run it:
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
