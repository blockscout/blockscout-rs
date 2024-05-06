DA Indexer Service
===

The DA Indexer service collects blobs from different DA solutions (currently only Celestia) and provides a convenient API for fetching blob data.

## Celestia
The Celestia indexer runs on top of the [Celestia light node](https://docs.celestia.org/nodes/light-node). It is worth noting that the indexer collects only blobs and some block metadata, it does not collect full blocks, transactions, etc.

## Env

| Variable                                                | Description                                            | Default value                    |
|---------------------------------------------------------|--------------------------------------------------------|----------------------------------|
| DA_INDEXER__DATABASE__CONNECT__URL                      | Postgres URL to stats db                               | ''                               |
| DA_INDEXER__DATABASE__CREATE_DATABASE                   | Boolean. Creates database on start                     | false                            |
| DA_INDEXER__DATABASE__RUN_MIGRATIONS                    | Boolean. Runs migrations on start                      | false                            |
| DA_INDEXER__CELESTIA_INDEXER__RPC__URL                  | Celestia light node RPC url                            | http://localhost:26658           |
| DA_INDEXER__CELESTIA_INDEXER__RPC__AUTH_TOKEN           | Celestia light node authorization token                | ''                               |
| DA_INDEXER__CELESTIA_INDEXER__CONCURRENCY               | Number of jobs processed concurrently                  | 1                                |
| DA_INDEXER__CELESTIA_INDEXER__START_HEIGHT              | The height of the block to start with                  | The local head of the light node |
| DA_INDEXER__CELESTIA_INDEXER__RESTART_DELAY             | The delay between attempts to restart the indexer      | 60 seconds                       |
| DA_INDEXER__CELESTIA_INDEXER__POLLING_INTERVAL          | The delay between polling for new blocks from the node | 12 seconds                       |
| DA_INDEXER__CELESTIA_INDEXER__RETRY_INTERVAL            | The delay between attempts to reprocess failed blocks  | 180 seconds                      |

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
