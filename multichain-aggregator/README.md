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

[anchor]: <> (anchors.envs.start)

| Variable                                           | Req&#x200B;uir&#x200B;ed | Description                        | Default value  |
| -------------------------------------------------- | ------------------------ | ---------------------------------- | -------------- |
| `MULTICHAIN_AGGREGATOR__DATABASE__CONNECT__URL`    | true                     | Postgres connect URL to service DB |                |
| `MULTICHAIN_AGGREGATOR__DATABASE__CREATE_DATABASE` |                          | Create database if doesn't exist   | `false`        |
| `MULTICHAIN_AGGREGATOR__DATABASE__RUN_MIGRATIONS`  |                          | Run database migrations            | `false`        |
| `MULTICHAIN_AGGREGATOR__METRICS__ADDR`             |                          | Metrics listen address             | `0.0.0.0:6060` |
| `MULTICHAIN_AGGREGATOR__METRICS__ENABLED`          |                          | Enable metrics endpoint            | `false`        |
| `MULTICHAIN_AGGREGATOR__METRICS__ROUTE`            |                          | Metrics route                      | `/metrics`     |
| `MULTICHAIN_AGGREGATOR__TRACING__ENABLED`          |                          | Enable tracing                     | `true`         |
| `MULTICHAIN_AGGREGATOR__TRACING__FORMAT`           |                          | Tracing format: `default`/`json`   | `default`      |

[anchor]: <> (anchors.envs.end)

## Troubleshooting

1. Invalid tonic version

```
`Router` and `Router` have similar names, but are actually distinct types
```

To fix this error you need to change tonic version of `tonic` in `blockscout-service-launcer` to `0.8`

For now you can only change in `Cargo.lock`
