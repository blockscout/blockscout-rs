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

Syntax for schedules specified in the config is parsed by rust `cron` crate, so refer to crate's [documentation or source code](https://docs.rs/cron/latest/cron/) for precise behaviour.

### Env

#### Service settings

Some variables are hidden in a disclosure widget below the table.

[anchor]: <> (anchors.envs.start.service)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `STATS__DB_URL` | | Postgres URL to stats db | `""` |
| `STATS__MULTICHAIN_MODE` | | Run stats service in multichain mode; modifying some settings and disabling regular charts | `false` |
| `STATS__INDEXER_DB_URL` | | Postgres URL to indexer db; renamed from `*_BLOCKSCOUT_DB_URL` | `null` |
| `STATS__BLOCKSCOUT_DB_URL` | | Postgres URL to blockscout db. Renamed to `*_INDEXER_DB_URL` but left for backwards-compatibility | `null` |
| `STATS__CREATE_DATABASE` | | Create database on start | `false` |
| `STATS__RUN_MIGRATIONS` | | Run migrations on start | `false` |
| `STATS__CHARTS_CONFIG` | | Path to config file for charts | `"config/charts.json"` |
| `STATS__LAYOUT_CONFIG` | | Path to config file for chart layout | `"config/layout.json"` |
| `STATS__UPDATE_GROUPS_CONFIG` | | Path to config file for update groups | `"config/update_groups.json"` |
| `STATS__MULTICHAIN_CHARTS_CONFIG` | | Path to config file for multichain charts (less priority over regular config) | `config/multichain/charts.json` |
| `STATS__MULTICHAIN_LAYOUT_CONFIG` | | Path to config file for multichain chart layout (less priority over regular config) | `config/multichain/layout.json` |
| `STATS__MULTICHAIN_UPDATE_GROUPS_CONFIG` | | Path to config file for multichain update groups (less priority over regular config) | `config/multichain/update_groups.json` |
| `STATS__FORCE_UPDATE_ON_START` | | Fully recalculate all charts on start | `false` |
| `STATS__CONCURRENT_START_UPDATES` | | Amount of concurrent charts update on start | `3` |
| `STATS__DEFAULT_SCHEDULE` | | Schedule used for update groups with no config | `"0 0 1 * * * *"` |
| `STATS__LIMITS__REQUESTED_POINTS_LIMIT` | | Maximum allowed number of requested points | `182500` |
| `STATS__BLOCKSCOUT_API_URL` | | URL to Blockscout API. Used for [conditional update start](#conditional-start). Required unless `STATS__IGNORE_BLOCKSCOUT_API_ABSENCE`  is set to `true`. | `null` |
| `STATS__CONDITIONAL_START__CHECK_PERIOD_SECS` | | Base time between start condition checking | `5` |
| `STATS__CONDITIONAL_START__BLOCKS_RATIO__ENABLED` | | Enable `blocks_ratio` threshold | `true` |
| `STATS__CONDITIONAL_START__BLOCKS_RATIO__THRESHOLD` | | Value for `blocks_ratio` threshold | `0.98` |
| `STATS__CONDITIONAL_START__INTERNAL_TRANSACTIONS_RATIO__ENABLED` | | Enable `internal_transactions_ratio` threshold | `true` |
| `STATS__CONDITIONAL_START__INTERNAL_TRANSACTIONS_RATIO__THRESHOLD` | | Value for `internal_transactions_ratio` threshold | `0.98` |
| `STATS__CONDITIONAL_START__USER_OPS_PAST_INDEXING_FINISHED__ENABLED` | | Enable checking user ops indexing status | `true` |
| `STATS__IGNORE_BLOCKSCOUT_API_ABSENCE` | | Disable requirement for blockscout api url setting. Turns off corresponding features if the api setting is not set | `false` |
| `STATS__DISABLE_INTERNAL_TRANSACTIONS` | | Disable functionality that utilizes internal transactions. In particular, disable internal transactions ratio check for starting the service and related charts (`newContracts`, `lastNewContracts`, and `contractsGrowth`). It has a higher priority than config files and respective envs. | `false` |
| `STATS__ENABLE_ALL_ARBITRUM` | | Enable Arbitrum-specific charts. Variable for convenience only, the same charts can be enabled one-by-one. | `false` |
| `STATS__ENABLE_ALL_OP_STACK` | | Enable OP-Stack-specific charts. Variable for convenience only, the same charts can be enabled one-by-one. | `false` |
| `STATS__ENABLE_ALL_EIP_7702` | | Enable EIP-7702-specific charts. Variable for convenience only, the same charts can be enabled one-by-one. | `false` |
| `STATS__API_KEYS__<KEY_NAME>` | | E.g. `very_secure_key_value`. Allows access to key-protected functinoality | `null` |

[anchor]: <> (anchors.envs.end.service)

##### Conditional start
In order to prevent incorrect statistics from being collected, there is an option to automatically delay chart update. This is controlled by `STATS_CONDITIONAL_START_*` environmental variables. 

The service will periodically check the enabled start conditions and start updating charts once they are satisfied.

<details><summary>Server settings</summary>
<p>

[anchor]: <> (anchors.envs.start.server)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `STATS__SERVER__GRPC__ADDR` | | Address for the gRPC server to listen on | `"0.0.0.0:8051"` |
| `STATS__SERVER__GRPC__ENABLED` | | Enable the gRPC server | `false` |
| `STATS__SERVER__HTTP__ADDR` | | Address for the HTTP server to listen on | `"0.0.0.0:8050"` |
| `STATS__SERVER__HTTP__CORS__ALLOWED_CREDENTIALS` | | Allow credentials in CORS requests | `true` |
| `STATS__SERVER__HTTP__CORS__ALLOWED_METHODS` | | List of allowed HTTP methods for CORS | `"PUT, GET, POST, OPTIONS, DELETE, PATCH"` |
| `STATS__SERVER__HTTP__CORS__ALLOWED_ORIGIN` | | Allowed origin for CORS requests | `""` |
| `STATS__SERVER__HTTP__CORS__BLOCK_ON_ORIGIN_MISMATCH` | | Block requests if origin does not match | `false` |
| `STATS__SERVER__HTTP__CORS__ENABLED` | | Enable CORS | `false` |
| `STATS__SERVER__HTTP__CORS__MAX_AGE` | | Max age for CORS preflight request caching (in seconds) | `3600` |
| `STATS__SERVER__HTTP__CORS__SEND_WILDCARD` | | Send wildcard for allowed origins in CORS | `false` |
| `STATS__SERVER__HTTP__ENABLED` | | Enable the HTTP server | `true` |
| `STATS__SERVER__HTTP__MAX_BODY_SIZE` | | Maximum allowed size for HTTP request bodies (in bytes) | `2097152` |
| `STATS__SERVER__HTTP__BASE_PATH` | | Path prefix to use before all services' endpoints. E.g. "/abcd" will make the service endpoints start with `/abcd/api/v1/...` instead of `/api/v1/...` | `null` |

[anchor]: <> (anchors.envs.end.server)

</p>
</details>

<details><summary>Tracing settings</summary>
<p>

[anchor]: <> (anchors.envs.start.tracing)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `STATS__JAEGER__AGENT_ENDPOINT` | | Jaeger agent endpoint for tracing | `"127.0.0.1:6831"` |
| `STATS__JAEGER__ENABLED` | | Enable Jaeger tracing | `false` |
| `STATS__TRACING__ENABLED` | | Enable tracing | `true` |
| `STATS__TRACING__FORMAT` | | Tracing format to use, either 'default' or 'json' | `"default"` |

[anchor]: <> (anchors.envs.end.tracing)

</p>
</details>

<details><summary>Metrics settings</summary>
<p>

[anchor]: <> (anchors.envs.start.metrics)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `STATS__METRICS__ADDR` | | Address for the metrics server to listen on | `"0.0.0.0:6060"` |
| `STATS__METRICS__ENABLED` | | Enable the metrics server | `false` |
| `STATS__METRICS__ROUTE` | | Route for exposing metrics | `"/metrics"` |

[anchor]: <> (anchors.envs.end.metrics)

</p>
</details>

#### Charts

[anchor]: <> (anchors.envs.start.charts)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `STATS_CHARTS__COUNTERS__<COUNTER_NAME>__DESCRIPTION` | | Counter `<COUNTER_NAME>` description, e.g. `"Some description"` | `null` |
| `STATS_CHARTS__COUNTERS__<COUNTER_NAME>__ENABLED` | | Enable counter `<COUNTER_NAME>`, e.g. `true` | `null` |
| `STATS_CHARTS__COUNTERS__<COUNTER_NAME>__TITLE` | | Displayed name of `<COUNTER_NAME>`, e.g. `"Some title with {{<variable_name>}}"` | `null` |
| `STATS_CHARTS__COUNTERS__<COUNTER_NAME>__UNITS` | | Measurement units for the counter, e.g. `"Bytes"` | `null` |
| `STATS_CHARTS__LINE_CHARTS__<LINE_CHART_NAME>__DESCRIPTION` | | Line chart `<LINE_CHART_NAME>` description, e.g. `"Some description with {{<variable_name>}}"` | `null` |
| `STATS_CHARTS__LINE_CHARTS__<LINE_CHART_NAME>__ENABLED` | | Enable `<LINE_CHART_NAME>`, e.g. `true` | `null` |
| `STATS_CHARTS__LINE_CHARTS__<LINE_CHART_NAME>__RESOLUTIONS__DAY` | | Enable daily data for the chart, e.g. `true` | `true` if the resolution is defined |
| `STATS_CHARTS__LINE_CHARTS__<LINE_CHART_NAME>__RESOLUTIONS__WEEK` | | Enable weekly data | `true` if defined |
| `STATS_CHARTS__LINE_CHARTS__<LINE_CHART_NAME>__RESOLUTIONS__MONTH` | | Enable monthly data | `true` if defined |
| `STATS_CHARTS__LINE_CHARTS__<LINE_CHART_NAME>__RESOLUTIONS__YEAR` | | Enable yearly data | `true` if defined |
| `STATS_CHARTS__LINE_CHARTS__<LINE_CHART_NAME>__TITLE` | | Displayed name of `<LINE_CHART_NAME>`, e.g. `"Some line chart title"` | `null` |
| `STATS_CHARTS__LINE_CHARTS__<LINE_CHART_NAME>__UNITS` | | Measurement units, e.g. `"{{<variable_name>}}"` | `null` |
| `STATS_CHARTS__TEMPLATE_VALUES__<VARIABLE_NAME>` | | Value to substitute instead of `{{<variable_name>}}`, e.g. `STATS_CHARTS__TEMPLATE_VALUES__NATIVE_COIN_SYMBOL="some_value"`. See full list of variables in charts config file (`charts.json`). | `null` |

[anchor]: <> (anchors.envs.end.charts)

#### Layout

[anchor]: <> (anchors.envs.start.layout)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `STATS_LAYOUT__COUNTERS_ORDER__<COUNTER_NAME>` | | Override position of `<COUNTER_NAME>` in the layout; `0` will place it first and `N` will place it Nth in the layout | `null` |
| `STATS_LAYOUT__LINE_CHART_CATEGORIES__<CATEGORY_NAME>__ORDER` | | Override position of `<CATEGORY_NAME>` in the layout | `null` |
| `STATS_LAYOUT__LINE_CHART_CATEGORIES__<CATEGORY_NAME>__CHARTS_ORDER__<LINE_CHART_NAME>` | | Override position of `<LINE_CHART_NAME>` within its category | `null` |
| `STATS_LAYOUT__LINE_CHART_CATEGORIES__<CATEGORY_NAME>__TITLE` | | Displayed name of the category, e.g. `"Accounts"` | `null` |

[anchor]: <> (anchors.envs.end.layout)

#### Update groups

[anchor]: <> (anchors.envs.start.groups)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `STATS_UPDATE_GROUPS__SCHEDULES__<UPDATE_GROUP_NAME>` | | Override update schedule of the group, e.g. `"0 0 */3 * * * *"` for update each 3 hours | `null` |

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