# Multichain Aggregator Service

This service is responsible for aggregating data from multiple Blockscout instances and providing a unified search API.

## Dev

- Install [just](https://github.com/casey/just) cli. Just is like make but better.
- Execute `just` to see avaliable dev commands

```bash
just
```

- Start dev postgres service by just typing

```bash
just start-postgres
```

- Now you ready to start API server! Just run it:

```
just run
```

## Envs

Service-specific environment variables. Common environment variables are listed [here](../docs/common-envs.md).

[anchor]: <> (anchors.envs.start)

| Variable                                                                      | Req&#x200B;uir&#x200B;ed | Description                                                 | Default value                                          |
| ----------------------------------------------------------------------------- | ------------------------ | ----------------------------------------------------------- | ------------------------------------------------------ |
| `MULTICHAIN_AGGREGATOR__DATABASE__CONNECT__URL`                               | true                     | e.g. `postgres://postgres:postgres@localhost:5432/postgres` |                                                        |
| `MULTICHAIN_AGGREGATOR__SERVICE__BENS_CLIENT__URL`                            | true                     | e.g. `http://localhost:8080/`                               |                                                        |
| `MULTICHAIN_AGGREGATOR__SERVICE__DAPP_CLIENT__URL`                            | true                     | e.g. `http://localhost:8080/`                               |                                                        |
| `MULTICHAIN_AGGREGATOR__SERVICE__TOKEN_INFO_CLIENT__URL`                      | true                     | e.g. `http://localhost:8080/`                               |                                                        |
| `MULTICHAIN_AGGREGATOR__DATABASE__CREATE_DATABASE`                            |                          |                                                             | `false`                                                |
| `MULTICHAIN_AGGREGATOR__DATABASE__RUN_MIGRATIONS`                             |                          |                                                             | `false`                                                |
| `MULTICHAIN_AGGREGATOR__REPLICA_DATABASE`                                     |                          |                                                             | `null`                                                 |
| `MULTICHAIN_AGGREGATOR__SERVICE__API__DEFAULT_PAGE_SIZE`                      |                          |                                                             | `50`                                                   |
| `MULTICHAIN_AGGREGATOR__SERVICE__API__MAX_PAGE_SIZE`                          |                          |                                                             | `100`                                                  |
| `MULTICHAIN_AGGREGATOR__SERVICE__BENS_PROTOCOLS`                              |                          |                                                             | `ens`                                                  |
| `MULTICHAIN_AGGREGATOR__SERVICE__DOMAIN_PRIMARY_CHAIN_ID`                     |                          |                                                             | `1`                                                    |
| `MULTICHAIN_AGGREGATOR__SERVICE__FETCH_CHAINS`                                |                          |                                                             | `false`                                                |
| `MULTICHAIN_AGGREGATOR__SERVICE__MARKETPLACE_ENABLED_CACHE_FETCH_CONCURRENCY` |                          |                                                             | `10`                                                   |
| `MULTICHAIN_AGGREGATOR__SERVICE__MARKETPLACE_ENABLED_CACHE_UPDATE_INTERVAL`   |                          |                                                             | `21600`                                                |
| `MULTICHAIN_AGGREGATOR__SERVICE__QUICK_SEARCH_CHAINS`                         |                          |                                                             | `1,8453,57073,698,109,7777777,100,10,42161,690,534352` |

[anchor]: <> (anchors.envs.end)