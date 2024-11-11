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

<!--
There are zero-width spaces added here and there to prevent too wide tables
by enabling word wrapping
-->

[anchor]: <> (anchors.envs.start.service)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `STATS__DB_URL` | | Postgres URL to stats db | `""` |
| `STATS__​BLOCKSCOUT_DB_URL` | | Postgres URL to blockscout db | `""` |
| `STATS__CREATE_DATABASE` | | Create database on start | `false` |
| `STATS__RUN_MIGRATIONS` | | Run migrations on start | `false` |
| `STATS__CHARTS_CONFIG` | | Path to config file for charts | `"config/charts.json"` |
| `STATS__LAYOUT_CONFIG` | | Path to config file for chart layout | `"config/layout.json"` |
| `STATS__UPDATE_​GROUPS_CONFIG` | | Path to config file for update groups | `"config/​update_groups.json"` |
| `STATS__SWAGGER_FILE` | | Path of the swagger file to serve in the swagger endpoint | `"../stats-proto/​swagger/stats.​swagger.yaml"` |
| `STATS__FORCE_​UPDATE_ON_START` | | Fully recalculate all charts on start | `false` |
| `STATS__CONCURRENT_​START_UPDATES` | | Amount of concurrent charts update on start | `3` |
| `STATS__​DEFAULT_​SCHEDULE` | | Schedule used for update groups with no config | `"0 0 1 * * * *"` |
| `STATS__LIMITS__REQUESTED_​POINTS_LIMIT` | | Maximum allowed number of requested points | `182500` |
| `STATS__BLOCKSCOUT_API_URL` | Required unless `STATS__​IGNORE_​​BLOCKSCOUT_​API_​ABSENCE` is set to `true`. | URL to Blockscout API. | `null` |
| `STATS__CONDITIONAL_​START__CHECK_PERIOD_SECS` | | Time between start condition checking (if they are not satisfied) | `5` |
| `STATS__CONDITIONAL_​START__BLOCKS_RATIO__​ENABLED` | | Enable `blocks_​ratio` threshold | `true` |
| `STATS__CONDITIONAL_​START__BLOCKS_RATIO__​THRESHOLD` | | Value for `blocks_​ratio` threshold | `0.98` |
| `STATS__CONDITIONAL_​START__INTERNAL_​TRANSACTIONS_RATIO__​ENABLED` | | Enable `internal_​transactions_​ratio` threshold | `true` |
| `STATS__CONDITIONAL_​START__INTERNAL_​TRANSACTIONS_RATIO__​THRESHOLD` | | Value for `internal_​transactions_​ratio` threshold | `0.98` |
| `STATS__IGNORE_​BLOCKSCOUT_API_ABSENCE` | | Disable requirement for blockscout api url setting. Turns off corresponding features if the api setting is not set | `false` |
| `STATS__DISABLE_​INTERNAL_TRANSACTIONS` | | Disable functionality that utilizes internal transactions. In particular, disable internal transactions ratio check for starting the service and related charts (`newContracts`, `lastNewContracts`, and `contractsGrowth`). It has a higher priority than config files and respective envs. | `false` |

[anchor]: <> (anchors.envs.end.service)

##### Conditional start
In order to prevent incorrect statistics from being collected, there is an option to automatically delay chart update. This is controlled by `STATS_CONDITIONAL_​START_*` environmental variables. 

The service will periodically check the enabled start conditions and start updating charts once they are satisfied.

<details><summary>Server settings</summary>
<p>

[anchor]: <> (anchors.envs.start.server)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `STATS__SERVER__​GRPC__ADDR` | | Address for the gRPC server to listen on | `"0.0.0.0:8051"` |
| `STATS__SERVER__​GRPC__ENABLED` | | Enable the gRPC server | `false` |
| `STATS__SERVER__​HTTP__ADDR` | | Address for the HTTP server to listen on | `"0.0.0.0:8050"` |
| `STATS__SERVER__​HTTP__CORS__​ALLOWED_CREDENTIALS` | | Allow credentials in CORS requests | `true` |
| `STATS__SERVER__​HTTP__CORS__​ALLOWED_METHODS` | | List of allowed HTTP methods for CORS | `"PUT, GET, POST, OPTIONS, DELETE, PATCH"` |
| `STATS__SERVER__​HTTP__CORS__​ALLOWED_ORIGIN` | | Allowed origin for CORS requests | `""` |
| `STATS__SERVER__​HTTP__CORS__​BLOCK_ON_ORIGIN_MISMATCH` | | Block requests if origin does not match | `false` |
| `STATS__SERVER__​HTTP__CORS__​ENABLED` | | Enable CORS | `false` |
| `STATS__SERVER__​HTTP__CORS__​MAX_AGE` | | Max age for CORS preflight request caching (in seconds) | `3600` |
| `STATS__SERVER__​HTTP__CORS__​SEND_WILDCARD` | | Send wildcard for allowed origins in CORS | `false` |
| `STATS__SERVER__​HTTP__ENABLED` | | Enable the HTTP server | `true` |
| `STATS__SERVER__​HTTP__MAX_BODY_SIZE` | | Maximum allowed size for HTTP request bodies (in bytes) | `2097152` |

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
| `STATS_CHARTS__​COUNTERS__<COUNTER_NAME>__​DESCRIPTION` | | Counter `<COUNTER_NAME>` description, e.g. `"Some description"` | `null` |
| `STATS_CHARTS__​COUNTERS__<COUNTER_NAME>__​ENABLED` | | Enable counter `<COUNTER_NAME>`, e.g. `true` | `null` |
| `STATS_CHARTS__​COUNTERS__<COUNTER_NAME>__​TITLE` | | Displayed name of `<COUNTER_NAME>`, e.g. `"Some title with {{<variable_name>}}"` | `null` |
| `STATS_CHARTS__​COUNTERS__<COUNTER_NAME>__​UNITS` | | Measurement units for the counter, e.g. `"Bytes"` | `null` |
| `STATS_CHARTS__​LINE_CHARTS__​<LINE_CHART_NAME>__​DESCRIPTION` | | Line chart `<LINE_CHART_NAME>` description, e.g. `"Some description with {{<variable_name>}}"` | `null` |
| `STATS_CHARTS__​LINE_CHARTS__​<LINE_CHART_NAME>__​ENABLED` | | Enable `<LINE_CHART_NAME>`, e.g. `true` | `null` |
| `STATS_CHARTS__​LINE_CHARTS__​<LINE_CHART_NAME>__​RESOLUTIONS__DAY` | | Enable daily data for the chart, e.g. `true` | `true` if the resolution is defined |
| `STATS_CHARTS__​LINE_CHARTS__​<LINE_CHART_NAME>__​RESOLUTIONS__WEEK` | | Enable weekly data | `true` if defined |
| `STATS_CHARTS__​LINE_CHARTS__​<LINE_CHART_NAME>__​RESOLUTIONS__MONTH` | | Enable monthly data | `true` if defined |
| `STATS_CHARTS__​LINE_CHARTS__​<LINE_CHART_NAME>__​RESOLUTIONS__YEAR` | | Enable yearly data | `true` if defined |
| `STATS_CHARTS__​LINE_CHARTS__​<LINE_CHART_NAME>__​TITLE` | | Displayed name of `<LINE_CHART_NAME>`, e.g. `"Some line chart title"` | `null` |
| `STATS_CHARTS__​LINE_CHARTS__​<LINE_CHART_NAME>__​UNITS` | | Measurement units, e.g. `"{{<variable_name>}}"` | `null` |
| `STATS_CHARTS__​TEMPLATE_VALUES__​<VARIABLE_NAME>` | | Value to substitute instead of `{{<variable_name>}}`, e.g. `STATS_CHARTS__​TEMPLATE_VALUES__​NATIVE_COIN_SYMBOL​="some_value"`. See full list of variables in charts config file (`charts.json`). | `null` |

[anchor]: <> (anchors.envs.end.charts)

#### Layout

[anchor]: <> (anchors.envs.start.layout)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `STATS_LAYOUT__​COUNTERS_ORDER__​<COUNTER_NAME>` | | Override position of `<COUNTER_NAME>` in the layout; `0` will place it first and `N` will place it Nth in the layout | `null` |
| `STATS_LAYOUT__​LINE_CHART_CATEGORIES__​<CATEGORY_NAME>__ORDER` | | Override position of `<CATEGORY_NAME>` in the layout | `null` |
| `STATS_LAYOUT__​LINE_CHART_CATEGORIES__​<CATEGORY_NAME>__​CHARTS_ORDER__​<LINE_CHART_NAME>` | | Override position of `<LINE_CHART_NAME>` within its category | `null` |
| `STATS_LAYOUT__​LINE_CHART_CATEGORIES__​<CATEGORY_NAME>__TITLE` | | Displayed name of the category, e.g. `"Accounts"` | `null` |

[anchor]: <> (anchors.envs.end.layout)

#### Update groups

[anchor]: <> (anchors.envs.start.groups)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `STATS_UPDATE_GROUPS__​SCHEDULES__<UPDATE_GROUP_NAME>` | | Override update schedule of the group, e.g. `"0 0 */3 * * * *"` for update each 3 hours | `null` |

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