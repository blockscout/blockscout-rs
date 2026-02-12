# Docker Compose Deployment

This directory contains a production-style deployment of **Interchain Indexer** using the pre-built image and local configuration files.

## Prerequisites

- Docker and Docker Compose
- A `.env` file in this directory with database credentials

## Quick Start

```bash
docker-compose up -d
```

Ensure a `.env` file exists

---

## 1. Tuning configuration: `chains.json` and `bridges.json`

Configuration is mounted from `./config` into the container. The service reads:

- **Chains** — `INTERCHAIN_INDEXER__CHAINS_CONFIG` (default in this setup: `/app/config/chains.json`)
- **Bridges** — `INTERCHAIN_INDEXER__BRIDGES_CONFIG` (default in this setup: `/app/config/bridges.json`)

Edit the files under `./config/` and restart the interchain-indexer service for changes to take effect.

It’s recommended to drop the database (if the service has been started before) when you modify the config files:

```bash
`docker-compose down -v`
```

### `config/chains.json`

Defines the blockchains the indexer knows about. Each entry describes one chain:

| Field        | Description |
| ------------ | ----------- |
| `chain_id`   | Numeric chain identifier (e.g. 43114 for Avalanche C-Chain). |
| `name`       | Human-readable chain name. |
| `native_id`  | Chain’s native/subnet id (hex), used for interchain routing. |
| `icon`       | Optional URL to chain icon. |
| `explorer`   | Optional explorer base URL and routes: `url`, `custom_tx_route`, `custom_address_route`, `custom_token_route`. |
| `rpcs`       | RPC config per chain. |

Add or remove chain objects to index more or fewer networks. RPC URLs must be reachable from inside the container.

### `config/bridges.json`

Defines which bridges (cross-chain mechanisms) to index. Each entry is one bridge:

| Field        | Description |
| ------------ | ----------- |
| `bridge_id`  | Unique numeric id for the bridge. |
| `name`       | Human-readable bridge name. |
| `type`       | Bridge type (e.g. `avalanche_native`). |
| `indexer`    | Indexer implementation (e.g. `AvalancheNative`). |
| `enabled`    | Whether this bridge is indexed. |
| `api_url` / `ui_url` | Optional external links. |
| `contracts`  | Per-chain contract config: `chain_id`, `address`, `version`, `started_at_block`. |

**`started_at_block`** — indexer starts scanning from this block on each chain; set it to reduce initial sync time or to start from a specific deployment block.

After editing JSON, restart the service:

```bash
docker-compose restart interchain-indexer
```

---

## 2. Database authentication (`.env`)

Database connection and auth are controlled by environment variables. The compose file uses `env_file: .env`, so you can keep secrets and overrides in a single place.

**Variables used for Postgres:**

| Variable                      | Description |
| ----------------------------- | ----------- |
| `POSTGRES_USER`               | PostgreSQL user (used by both the database container and the indexer). |
| `POSTGRES_PASSWORD`           | PostgreSQL password. |
| `POSTGRES_HOST_AUTH_METHOD`   | Optional; default in compose is `scram-sha-256`. |

The indexer’s connection URL is set in the compose file as:

`postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@database:5432/interchain_indexer`

So changing `POSTGRES_USER` and `POSTGRES_PASSWORD` in `.env` is enough to tune database auth. Ensure the same `.env` is used when you run `docker compose` (from this directory or with `-f docker/docker-compose.yml`).

**Example `.env` in the `docker` folder:**

```env
POSTGRES_USER=postgres
POSTGRES_PASSWORD=admin
```

---

## 3. Usage: API, Swagger, and metrics

Once the stack is running:

- **HTTP API** — `http://localhost:8050`
- **gRPC** — `localhost:8051` (disabled by default)
- **Prometheus metrics** — `http://localhost:6060/metrics`

### REST API and Swagger

- **Swagger UI / OpenAPI spec**  
  Open in a browser:  
  **http://localhost:8050/api/v1/docs/swagger.yaml**  
  (Use this URL in Swagger Editor or any OpenAPI tool; the service may also expose a Swagger UI path — check the same base path under `/api/v1/docs/` if available.)

- **Main REST endpoints** (see Swagger for full request/response schemas):
  - **GET** `/api/v1/interchain/messages` — paginated cross-chain messages (optional query: `page_size`, `page_token`, etc.).
  - **GET** `/api/v1/interchain/transfers` — paginated cross-chain transfers.

- **Health / status**  
  Use the paths defined in the Swagger spec (e.g. health or status endpoints) under `http://localhost:8050`.

### Stopping

```bash
docker-compose down
```

Data is persisted in the `./database` volume (relative to the compose file). Remove the volume as well to start with a clean DB: `docker-compose down -v` (only if you intend to drop all data).
