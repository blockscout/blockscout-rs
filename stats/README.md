# <h1 align="center"> Statistics </h1>

**Stats (Statistics)** - is a service designed to calculate and present statistical information from a Blockscout instance. This service establishes a connection with the Blockscout database and periodically updates a collection of charts, including lines and counters, based on a predefined schedule. The calculated data is then made available through a REST API, allowing users to access and utilize the statistical information.

The service consists of 2 parts, a stats calculation library and a transport layer that serves requests:

+ [stats](./stats) - implements actual chart calculation logic as a library and exposes an interface to be used by the transport layer;
+ A transport layer that implements some APIs over the service ([stats-server](./stats-server/)).

## Build

### Using docker

+ You can build the provided sources using [Dockerfile](./Dockerfile)

+ Alternatively, you can use docker images from our [registry](https://github.com/blockscout/blockscout-rs/pkgs/container/stats)

### Using docker-compose

+ You can use docker-compose.yaml written in [blockscout main repo](https://github.com/blockscout/blockscout/blob/master/docker-compose/services/docker-compose-stats.yml) to run latest version of stats with database

### Building from source

```console
cargo install --git https://github.com/blockscout/blockscout-rs stats-server
stats-server
```

## Config

### Env

| Variable                        | Description                                          | Default value        |
| ------------------------------- | ---------------------------------------------------- | -------------------- |
| STATS__DB_URL                   | Postgres URL to stats db                             | ''                   |
| STATS__BLOCKSCOUT_DB_URL        | Postgres URL to blockscout db                        | ''                   |
| STATS__CREATE_DATABASE          | Boolean. Creates database on start                   | false                |
| STATS__RUN_MIGRATIONS           | Boolean. Runs migrations on start                    | false                |
| STATS__CHARTS_CONFIG            | Path to charts.toml config file                      | ./config/charts.toml |
| STATS__FORCE_UPDATE_ON_START    | Boolean. Fully recalculates all charts on start      | false                |
| STATS__CONCURRENT_START_UPDATES | Integer. Amount of concurrent charts update on start | 3                    |

### Charts config

Blockscout provides a collection of predefined charts to visualize statistics. You can enable or disable these charts by modifying the charts.toml file. The default configuration for the charts can be found [here](./config/charts.toml). You can use this file as a template for customization.

To remove unnecessary or unrelated charts, simply open the `charts.toml` file and delete the corresponding chart entries. In addition to modifying the `charts.toml` file, it is important to provide the `STATS__CHARTS_CONFIG` variable with the path to the updated configuration file.

## For development

+ Install [docker](https://docs.docker.com/engine/install/), [rust](https://www.rust-lang.org/tools/install), [just](https://github.com/casey/just)

+ Start dev postgres:

```console
just start-postgres
```

+ Start blockscout instance with varialbe `DATABASE_URL=postgres://postgres:admin@host.docker.internal:5432/blockscout`

+ Start stats server:

```console
export STATS__RUN_MIGRATIONS=true
export STATS__DB_URl="postgres://postgres:admin@localhost:5432/stats"
export STATS__BLOCKSCOUT_DB_URL="postgres://postgres:admin@localhost:5432/blockscout" 
cargo run --bin stats-server
```
