# <h1 align="center"> Statistics </h1>

**Stats (Statistics)** - is a service designed to calculate and present statistical information from a Blockscout instance. This service establishes a connection with the Blockscout database and periodically updates a collection of charts, including lines and counters, based on a predefined schedule. The calculated data is then made available through a REST API, allowing users to access and utilize the statistical information.

The service consists of 2 parts, a stats calculation library and a transport layer that serves requests:

+ [stats](./stats) - implements actual chart calculation logic as a library and exposes an interface to be used by the transport layer;
+ A transport layer that implements some APIs over the service ([stats-server](./stats-server/)).

## Requirements

- Postgresql database for this service
- Access to Blockscout database

## Build

### Using docker

+ You can build the provided sources using [Dockerfile](./Dockerfile)

+ Alternatively, you can use docker images from our [registry](https://github.com/blockscout/blockscout-rs/pkgs/container/stats)

### Using docker-compose

+ You can use compose file from [blockscout main repo](https://github.com/blockscout/blockscout/blob/master/docker-compose/services/stats.yml) to run latest version of stats with database

### Building from source

```console
cargo install --git https://github.com/blockscout/blockscout-rs stats-server
stats-server
```

## Config

The microservice can be controlled using configuration files and environmental variables. Envs have a priority over files.

### Config files

Blockscout provides a collection of predefined charts to visualize statistics. You can enable or disable these charts by modifying the `charts.json` file. Layout of the charts (which is returned by the server) is configurable in `layout.json`. Schedule of updated charts for each group is set up in `update_groups.json`.

The default configurations can be found [here](./config/). You can use these files as a base for customization.

If non-default config files are used, respective environment variables (e.g. `STATS__CHARTS_CONFIG`) should be set to the new files.

#### Charts parameters

To disable unnecessary charts, open the `charts.json` file and set `enabled: false` for them. Other parameters can also be set/modified there.

#### Layout configuration

Categories for line charts, category metadata, and chart order within category are set in `layout.json`.

#### Update groups config

Charts dependant on each other are combined in update groups. Charts within one update group are updated **together** according to their dependency relations. Updates are scheduled for each such group in `update_groups.json` file.


### Env

#### Service settings

Some variables are hidden in a disclosure widget below the table.

[anchor]: <> (anchors.envs.start.service)
[anchor]: <> (anchors.envs.end.service)

<details><summary>Server settings</summary>
<p>

[anchor]: <> (anchors.envs.start.server)
[anchor]: <> (anchors.envs.end.server)

</p>
</details>

<details><summary>Tracing settings</summary>
<p>

[anchor]: <> (anchors.envs.start.tracing)
[anchor]: <> (anchors.envs.end.tracing)

</p>
</details>

<details><summary>Metrics settings</summary>
<p>

[anchor]: <> (anchors.envs.start.metrics)
[anchor]: <> (anchors.envs.end.metrics)

</p>
</details>

| Variable                        | Description                                          | Default value               |
| ------------------------------- | ---------------------------------------------------- | --------------------------- |
| STATS__DB_URL                   | Postgres URL to stats db                             | ''                          |
| STATS__BLOCKSCOUT_DB_URL        | Postgres URL to blockscout db                        | ''                          |
| STATS__CREATE_DATABASE          | Boolean. Creates database on start                   | false                       |
| STATS__RUN_MIGRATIONS           | Boolean. Runs migrations on start                    | false                       |
| STATS__CHARTS_CONFIG            | Path to `charts.json` config file                    | ./config/charts.json        |
| STATS__LAYOUT_CONFIG            | Path to `layout.json` config file                    | ./config/layout.json        |
| STATS__UPDATE_GROUPS_CONFIG     | Path to `update_groups.json` config file             | ./config/update_groups.json |
| STATS__FORCE_UPDATE_ON_START    | Boolean. Fully recalculates all charts on start      | false                       |
| STATS__CONCURRENT_START_UPDATES | Integer. Amount of concurrent charts update on start | 3                           |

#### Charts

[anchor]: <> (anchors.envs.start.charts)
[anchor]: <> (anchors.envs.end.charts)

#### Layout

[anchor]: <> (anchors.envs.start.layout)

| Variable | Required | Description | Default value |
| --- | --- | --- | --- |
| `STATS_LAYOUT__COUNTERS_ORDER__<COUNTER_NAME>` | | e.g. `3` | `null` |
| `STATS_LAYOUT__LINE_CHART_CATEGORIES__<CATEGORY_NAME>__CHARTS_ORDER__<LINE_CHART_NAME>` | | e.g. `1` | `null` |
| `STATS_LAYOUT__LINE_CHART_CATEGORIES__<CATEGORY_NAME>__ORDER` | | | `null` |
| `STATS_LAYOUT__LINE_CHART_CATEGORIES__<CATEGORY_NAME>__TITLE` | | e.g. `"Accounts"` | `null` |

[anchor]: <> (anchors.envs.end.layout)

#### Update groups

[anchor]: <> (anchors.envs.start.groups)

| Variable | Required | Description | Default value |
| --- | --- | --- | --- |
| `STATS_UPDATE_GROUPS__SCHEDULES__<UPDATE_GROUP_NAME>` | | e.g. `"0 0 */3 * * * *"` | `null` |

[anchor]: <> (anchors.envs.end.groups)

## For development

### Manual run

+ Install [docker](https://docs.docker.com/engine/install/), [rust](https://www.rust-lang.org/tools/install), [just](https://github.com/casey/just)

+ Start dev postgres:

```console
just start-postgres
```

+ Start blockscout instance with varialbe `DATABASE_URL=postgres://postgres:admin@host.docker.internal:5432/blockscout`

+ Start stats server:

```console
export STATS__RUN_MIGRATIONS=true
export STATS__DB_URL="postgres://postgres:admin@localhost:5432/stats"
export STATS__BLOCKSCOUT_DB_URL="postgres://postgres:admin@localhost:5432/blockscout" 
cargo run --bin stats-server
```

### Docker compose

Alternatively, you can use `docker-compose.dev.yml` for simplicity.

+ Set `ETHEREUM_JSONRPC_HTTP_URL` and `ETHEREUM_JSONRPC_TRACE_URL` to ethereum node you have access to (in `backend` (Blockscout) service)
+ Set `FIRST_BLOCK` to some recent block for less load on the node
+ Update `ETHEREUM_JSONRPC_VARIANT` if necessary
+ Run `docker compose -f docker-compose.dev.yml up -d`
