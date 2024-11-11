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

| Variable                                           | Req&#x200B;uir&#x200B;ed | Description                        | Default value  |
| -------------------------------------------------- | ------------------------ | ---------------------------------- | -------------- |
| `MULTICHAIN_AGGREGATOR__DATABASE__CONNECT__URL`    | true                     | Postgres connect URL to service DB |                |
| `MULTICHAIN_AGGREGATOR__DATABASE__CREATE_DATABASE` |                          | Create database if doesn't exist   | `false`        |
| `MULTICHAIN_AGGREGATOR__DATABASE__RUN_MIGRATIONS`  |                          | Run database migrations            | `false`        |

[anchor]: <> (anchors.envs.end)
