Scoutcloud Service
===

Scoutcloud provides API to deploy and manage blockscout instances. 
It tracks amount of time each instance is running and charges user for it.

## Envs

[anchor]: <> (anchors.envs.start)

| Variable                                | Is required | Example value                                          | Comment |
|-----------------------------------------|-------------|--------------------------------------------------------|---------|
| `SCOUTCLOUD__DATABASE__CONNECT__URL`    | true        | `postgres://postgres:postgres@localhost:5432/postgres` |         |
| `SCOUTCLOUD__GITHUB__OWNER`             | true        | `blockscout`                                           |         |
| `SCOUTCLOUD__GITHUB__REPO`              | true        | `autodeploy`                                           |         |
| `SCOUTCLOUD__GITHUB__TOKEN`             | true        | `your_github_token`                                    |         |
| `SCOUTCLOUD__DATABASE__CREATE_DATABASE` |             | `false`                                                |         |
| `SCOUTCLOUD__DATABASE__RUN_MIGRATIONS`  |             | `false`                                                |         |
| `SCOUTCLOUD__GITHUB__BRANCH`            |             | `master`                                               |         |
| `SCOUTCLOUD__METRICS__ADDR`             |             | `0.0.0.0:6060`                                         |         |
| `SCOUTCLOUD__METRICS__ENABLED`          |             | `false`                                                |         |
| `SCOUTCLOUD__METRICS__ROUTE`            |             | `/metrics`                                             |         |
| `SCOUTCLOUD__TRACING__ENABLED`          |             | `true`                                                 |         |
| `SCOUTCLOUD__TRACING__FORMAT`           |             | `default`                                              |         |

[anchor]: <> (anchors.envs.end)

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

+ Now you ready to start API server! Just run it:
```bash
just run
# or if you want to export envs from .env file
dotenv -e .env -- just run
```

## Troubleshooting

1. Invalid tonic version

```
`Router` and `Router` have similar names, but are actually distinct types
```

To fix this error you need to change tonic version of `tonic` in `blockscout-service-launcer` to `0.8`

For now you can only change in `Cargo.lock`