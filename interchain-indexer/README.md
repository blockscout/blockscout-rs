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

## Envs

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
