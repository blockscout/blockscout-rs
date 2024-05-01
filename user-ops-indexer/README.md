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

## Build

### Using docker

Service is built using docker (see [Dockerfile](./Dockerfile)). Public build images are available
through [GitHub Container Registry](https://github.com/blockscout/blockscout-rs/pkgs/container/user-ops-indexer)

## Config

### Env

| Variable                                                        | Description                                                                                                                                                                                                         | Required | Default value         |
|-----------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|----------|-----------------------|
| `USER_OPS_INDEXER__SERVER__HTTP__ENABLED`                       | Enable HTTP API server                                                                                                                                                                                              |          | `true`                |
| `USER_OPS_INDEXER__SERVER__HTTP__ADDR`                          | HTTP API listening interface                                                                                                                                                                                        |          | `0.0.0.0:8050`        |
| `USER_OPS_INDEXER__SERVER__HTTP__MAX_BODY_SIZE`                 | Max HTTP body size for incoming API requests                                                                                                                                                                        |          | `2097152`             |
| `USER_OPS_INDEXER__SERVER__GRPC__ENABLED`                       | Enable GRPC API server                                                                                                                                                                                              |          | `false`               |
| `USER_OPS_INDEXER__SERVER__GRPC__ADDR`                          | GRPC API listening interface                                                                                                                                                                                        |          | `0.0.0.0:8051`        |
| `USER_OPS_INDEXER__API__MAX_PAGE_SIZE`                          | Max page size for API requests                                                                                                                                                                                      |          | `100`                 |
| `USER_OPS_INDEXER__INDEXER__RPC_URL`                            | Indexer RPC URL, should be an archive JSON RPC node with `eth`, `web3` and `trace`/`debug` namespaces enabled. Both HTTP and WS protocols are supported. WS is recommended for local RPC nodes, use HTTP otherwise. | true     | `ws://127.0.0.1:8546` |
| `USER_OPS_INDEXER__INDEXER__CONCURRENCY`                        | Indexer concurrency. Will process up to the configured number of transactions concurrently                                                                                                                          |          | `10`                  |
| `USER_OPS_INDEXER__INDEXER__ENTRYPOINTS__V06`                   | Enable Entrypoint v0.6 indexer                                                                                                                                                                                      |          | `true`                |
| `USER_OPS_INDEXER__INDEXER__ENTRYPOINTS__V07`                   | Enable Entrypoint v0.7 indexer                                                                                                                                                                                      |          | `true`                |
| `USER_OPS_INDEXER__INDEXER__REALTIME__ENABLED`                  | Enable forward realtime indexing of user operations from the `latest` block                                                                                                                                         |          | `true`                |
| `USER_OPS_INDEXER__INDEXER__PAST_RPC_LOGS_INDEXER__ENABLED`     | Enable one-time reindex of missed user operations from recent blocks                                                                                                                                                |          | `false`               |
| `USER_OPS_INDEXER__INDEXER__PAST_RPC_LOGS_INDEXER__BLOCK_RANGE` | Block range width for missed user operations reindex. Will re-index events from a given number of blocks prior the `latest` block                                                                                   |          | `0`                   |
| `USER_OPS_INDEXER__INDEXER__PAST_DB_LOGS_INDEXER__ENABLED`      | Enable one-time reindex of missed user operations from core Blockscout DB. Will query relevant events from `logs` Postgres table                                                                                    |          | `false`               |
| `USER_OPS_INDEXER__INDEXER__PAST_DB_LOGS_INDEXER__START_BLOCK`  | Block range start for one-time DB reindex. Use positive number for static block number, or zero/negative number to count backwards from `latest`                                                                    |          | `0`                   |
| `USER_OPS_INDEXER__INDEXER__PAST_DB_LOGS_INDEXER__END_BLOCK`    | Block range end for one-time DB reindex. Use positive number for static block number, or zero/negative number to count backwards from `latest`                                                                      |          | `0`                   |
| `USER_OPS_INDEXER__DATABASE__CONNECT__URL`                      | Postgres connect URL to Blockscout DB with read/write access                                                                                                                                                        | true     | (empty)               |
| `USER_OPS_INDEXER__DATABASE__CREATE_DATABASE`                   | Create database if doesn't exist                                                                                                                                                                                    |          | `false`               |
| `USER_OPS_INDEXER__DATABASE__RUN_MIGRATIONS`                    | Run database migrations                                                                                                                                                                                             |          | `false`               |
| `USER_OPS_INDEXER__METRICS__ENABLED`                            | Enable metrics collection endpoint                                                                                                                                                                                  |          | `false`               |
| `USER_OPS_INDEXER__METRICS__ADDR`                               | Metrics collection listening interface                                                                                                                                                                              |          | `0.0.0.0:6060`        |
| `USER_OPS_INDEXER__METRICS__ROUTE`                              | Metrics collection API route                                                                                                                                                                                        |          | `/metrics`            |
| `USER_OPS_INDEXER__JAEGER__ENABLED`                             | Enable Jaeger tracing                                                                                                                                                                                               |          | `false`               |
| `USER_OPS_INDEXER__JAEGER__AGENT_ENDPOINT`                      | Jaeger tracing listening interface                                                                                                                                                                                  |          | `127.0.0.1:6831`      |
| `USER_OPS_INDEXER__TRACING__ENABLED`                            | Enable tracing log module                                                                                                                                                                                           |          | `true`                |
| `USER_OPS_INDEXER__TRACING__FORMAT`                             | Tracing format. `default` / `json`                                                                                                                                                                                  |          | `default`             |

## Running

### Locally

For testing and development purposes, service can be run locally, without having an active instance of Blockscout Elixir
backend, but it'll need access to the working JSON RPC URL and to the fully indexed Blockscout database (tables `blocks`
and `logs` in particular).

Configure env as described above and then run the service as following:

```shell
cargo run --bin user-ops-indexer-server
```

### Production

Production setup should also include necessary Elixir
backend [configuration](https://github.com/blockscout/docs/blob/master/for-developers/information-and-settings/env-variables.md#blockscout-account-abstraction)
and
frontend [configuration](https://github.com/blockscout/frontend/blob/main/docs/ENVS.md#user-operations-feature-erc-4337)

Running using docker
or [Blockscout Stack Helm charts](https://docs.blockscout.com/for-developers/deployment/kubernetes-deployment) is
recommended.
