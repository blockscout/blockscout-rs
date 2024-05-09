# <h1 align="center"> User Ops Indexer </h1>

**User Ops Indexer** is a service designed to index, decode and serve user operations as per the ERC-4337 standard.

The service can index 2 official ERC-4337 Entrypoint deployments:

* v0.6
  Entrypoint - [0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789](https://eth.blockscout.com/address/0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789)
* v0.7
  Entrypoint - [0x0000000071727De22E5E9d8BAf0edAc6f37da032](https://eth.blockscout.com/address/0x0000000071727De22E5E9d8BAf0edAc6f37da032)

The service consists of 2 parts:

* [Indexer logic](./user-ops-indexer-logic) - entrypoint contract indexing module. Each entrypoint contract is
  indexed in a separate tokio async task.
* [API server](./user-ops-indexer-server) - API module serving data about indexed user operations, accounts, factories,
  bundlers.

## Requirements

No additional dependencies

## How to run

### Production

Set the following ENVs on the Blockscout
instance ([configuration](https://github.com/blockscout/docs/blob/master/for-developers/information-and-settings/env-variables.md#blockscout-account-abstraction)):

* `MICROSERVICE_ACCOUNT_ABSTRACTION_ENABLED=true`
* `MICROSERVICE_ACCOUNT_ABSTRACTION_URL={service_url}`

And the following ENVs on the Blockscout
frontend ([configuration](https://github.com/blockscout/frontend/blob/main/docs/ENVS.md#user-operations-feature-erc-4337)):

* `NEXT_PUBLIC_HAS_USER_OPS=true`

It's recommended to run all the services
using [docker compose](https://github.com/blockscout/blockscout/tree/master/docker-compose)
or [Blockscout Stack Helm charts](https://docs.blockscout.com/for-developers/deployment/kubernetes-deployment).

### Locally

For testing and development purposes, service can be run locally, without having an active instance of Blockscout Elixir
backend, but it'll need access to the working JSON RPC URL and to the fully indexed Blockscout database (tables `blocks`
and `logs` in particular).

Configure env as described above and then run the service as following:

```shell
cargo run --bin user-ops-indexer-server
```

## Envs

Here, we describe variables specific to this service. Variables common to all services can be
found [here](../docs/common-envs.md).

| Variable                                                        | Required | Description                                                                                                                                                                                                         | Default value         |
|-----------------------------------------------------------------|----------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------|
| `USER_OPS_INDEXER__API__MAX_PAGE_SIZE`                          |          | Max page size for API requests                                                                                                                                                                                      | `100`                 |
| `USER_OPS_INDEXER__INDEXER__RPC_URL`                            | true     | Indexer RPC URL, should be an archive JSON RPC node with `eth`, `web3` and `trace`/`debug` namespaces enabled. Both HTTP and WS protocols are supported. WS is recommended for local RPC nodes, use HTTP otherwise. | `ws://127.0.0.1:8546` |
| `USER_OPS_INDEXER__INDEXER__CONCURRENCY`                        |          | Indexer concurrency. Will process up to the configured number of transactions concurrently                                                                                                                          | `10`                  |
| `USER_OPS_INDEXER__INDEXER__ENTRYPOINTS__V06`                   |          | Enable Entrypoint v0.6 indexer                                                                                                                                                                                      | `true`                |
| `USER_OPS_INDEXER__INDEXER__ENTRYPOINTS__V07`                   |          | Enable Entrypoint v0.7 indexer                                                                                                                                                                                      | `true`                |
| `USER_OPS_INDEXER__INDEXER__REALTIME__ENABLED`                  |          | Enable forward realtime indexing of user operations from the `latest` block                                                                                                                                         | `true`                |
| `USER_OPS_INDEXER__INDEXER__PAST_RPC_LOGS_INDEXER__ENABLED`     |          | Enable one-time reindex of missed user operations from recent blocks                                                                                                                                                | `false`               |
| `USER_OPS_INDEXER__INDEXER__PAST_RPC_LOGS_INDEXER__BLOCK_RANGE` |          | Block range width for missed user operations reindex. Will re-index events from a given number of blocks prior the `latest` block                                                                                   | `0`                   |
| `USER_OPS_INDEXER__INDEXER__PAST_DB_LOGS_INDEXER__ENABLED`      |          | Enable one-time reindex of missed user operations from core Blockscout DB. Will query relevant events from `logs` Postgres table                                                                                    | `false`               |
| `USER_OPS_INDEXER__INDEXER__PAST_DB_LOGS_INDEXER__START_BLOCK`  |          | Block range start for one-time DB reindex. Use positive number for static block number, or zero/negative number to count backwards from `latest`                                                                    | `0`                   |
| `USER_OPS_INDEXER__INDEXER__PAST_DB_LOGS_INDEXER__END_BLOCK`    |          | Block range end for one-time DB reindex. Use positive number for static block number, or zero/negative number to count backwards from `latest`                                                                      | `0`                   |
| `USER_OPS_INDEXER__DATABASE__CONNECT__URL`                      | true     | Postgres connect URL to Blockscout DB with read/write access                                                                                                                                                        | (empty)               |
| `USER_OPS_INDEXER__DATABASE__CREATE_DATABASE`                   |          | Create database if doesn't exist                                                                                                                                                                                    | `false`               |
| `USER_OPS_INDEXER__DATABASE__RUN_MIGRATIONS`                    |          | Run database migrations                                                                                                                                                                                             | `false`               |

## Links

- [Swagger](https://blockscout.github.io/swaggers/services/user-ops-indexer/index.html)
- [Packages](https://github.com/blockscout/blockscout-rs/pkgs/container/user-ops-indexer)
- [Releases](https://github.com/blockscout/blockscout-rs/releases?q=user-ops-indexer&expanded=true)
