Scoutcloud Service
===

Scoutcloud provides API to deploy and manage blockscout instances. 
It tracks amount of time each instance is running and charges user for it.

## Envs

[anchor]: <> (anchors.envs.start)

| Variable                                | Required | Description                                         | Default value  |
|-----------------------------------------|----------|-----------------------------------------------------|----------------|
| `SCOUTCLOUD__DATABASE__CONNECT__URL`    | true     | URL for connecting to the database.                 |                |
| `SCOUTCLOUD__GITHUB__OWNER`             | true     | GitHub owner or organization name.                  |                |
| `SCOUTCLOUD__GITHUB__REPO`              | true     | GitHub repository name.                             |                |
| `SCOUTCLOUD__GITHUB__TOKEN`             | true     | GitHub personal access token for authentication.    |                |
| `SCOUTCLOUD__DATABASE__CREATE_DATABASE` |          | Whether to create the database if it doesn't exist. | `false`        |
| `SCOUTCLOUD__DATABASE__RUN_MIGRATIONS`  |          | Whether to run database migrations.                 | `false`        |
| `SCOUTCLOUD__GITHUB__BRANCH`            |          | GitHub branch name                                  | `main`         |
| `SCOUTCLOUD__METRICS__ADDR`             |          | Address for metrics collection.                     | `0.0.0.0:6060` |
| `SCOUTCLOUD__METRICS__ENABLED`          |          | Whether metrics collection is enabled.              | `false`        |
| `SCOUTCLOUD__METRICS__ROUTE`            |          | Route for metrics collection API.                   | `/metrics`     |
| `SCOUTCLOUD__TRACING__ENABLED`          |          | Whether tracing is enabled.                         | `true`         |
| `SCOUTCLOUD__TRACING__FORMAT`           |          | Format for tracing. `default`/`json`                    | `default`      |

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