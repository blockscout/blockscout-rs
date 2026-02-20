# Docker Compose Deployment

This directory contains a production-style deployment of **Interchain Indexer** using the pre-built image and local configuration files.

**NOTE** These instructions are only related to the main compose file (`docker-compose.yml`). Another one, `docker-compose-ubi.yml`, is intended to set up a single Universal Bridge Indexer instance without backend/frontend (moreover, it uses the top-level `config` folder to load appropriate bridges/chains configuration).

## Prerequisites

- Docker and Docker Compose

## Quick Start

```bash
docker-compose up -d
```

---

## 1. Tuning Universal Bridge Indexer configuration: `chains.json` and `bridges.json`

Configuration is mounted from `./config` into the container. The service reads:

- **Chains** — `INTERCHAIN_INDEXER__CHAINS_CONFIG` (default in this setup: `/app/config/chains.json`)
- **Bridges** — `INTERCHAIN_INDEXER__BRIDGES_CONFIG` (default in this setup: `/app/config/bridges.json`)

Edit the files under `./config/` and restart the interchain-indexer service for changes to take effect.

It’s recommended to drop the database (if the service has been started before) when you modify the config files:

```bash
docker-compose down -v
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

**`started_at_block`** — indexer starts scanning from this block on associated chain; set it to reduce initial sync time or to start from a specific deployment block.

## 2. Tuning backend/frontend configuration: `docker-compose.yml`

Unlike the Universal Bridge Indexer, which can index messages across multiple blockchains, the frontend and backend must be configured for a single network. To do this, adjust the following environment variables in the `docker-compose.yml` file:

### For `backend`:
- `ETHEREUM_JSONRPC_HTTP_URL`, `ETHEREUM_JSONRPC_TRACE_URL` - RPC node URL
- `FIRST_BLOCK`, `TRACE_FIRST_BLOCK` - the block from which to start indexing the chain. Please note that it is advisable to set these variables for long-established networks to reduce synchronization time and reduce the load on the node. For newly created networks, these variables can be set to zero or removed.

### For `frontend`:
- `FAVICON_MASTER_URL` - Explorer tab icon
- `NEXT_PUBLIC_NETWORK_NAME` - Chain name
- `NEXT_PUBLIC_NETWORK_ID` - EVM Chain ID
- `NEXT_PUBLIC_NETWORK_RPC_URL` - RPC node URL
- `NEXT_PUBLIC_NETWORK_CURRENCY_NAME` - Native token name
- `NEXT_PUBLIC_NETWORK_CURRENCY_SYMBOL` - Native token symbol

### For `interchain-indexer`:
- `INTERCHAIN_INDEXER__TOKEN_INFO__BLOCKSCOUT_TOKEN_INFO__IGNORE_CHAINS` - It is recommended to set the current EVM chain ID to exclude token info requests for it. The token info service will most likely not contain token information for a newly created L1.

---

## 3. Starting the containers

Provide GitHub credentials to access the `ghcr.io` repository (to pull private images). Use your access token as a password.

```bash
docker login ghcr.io -u YOUR_GITHUB_USERNAME
```

Start the containers:

```bash
docker-compose up -d
```

You can observe container logs by entering the following command:

```bash
docker-compose logs -f SERVICE_NAME
```

...where `SERVICE_NAME` is one of the following (leave it empty to observe full logs):
- `db`: Postgres database
- `backend`: the main Blockscout API Backend
- `interchain-indexer`: Universal Bridge Indexer
- `frontend`: Blockscout frontend web-server

## 4. Usage: Frontend, API, Swagger, and metrics

When the stack is running, you can access the frontend by entering `http://localhost` in your browser.

Please keep in mind it could take some time for the web server to start up (depending on your host machine performance).

### REST API and Swagger

Additionally Universal Bridge Indexer resources are mapped to the `8050` TCP port:

- **Swagger UI / OpenAPI spec**  
  Open in a browser:  
  **http://localhost:8050/api/v1/docs/swagger.yaml**  
  (Use this URL in Swagger Editor or any OpenAPI tool; the service may also expose a Swagger UI path — check the same base path under `/api/v1/docs/` if available.)

- **Main REST endpoints** (see Swagger for full request/response schemas):
  - **GET** `http://localhost:8050/api/v1/interchain/messages` — paginated cross-chain messages (optional query: `page_size`, `page_token`, etc.).
  - **GET** `http://localhost:8050/api/v1/interchain/transfers` — paginated cross-chain transfers.

- **The simplest counters** (will be moved to the separate stats service soon):
  - **GET** `http://localhost:8050/api/v1/stats/common` — total indexed messages and transfers.
  - **GET** `http://localhost:8050/api/v1/stats/daily` — daily indexed messages and transfers.

- **Health / Status endpoints**
  - **GET** `http://localhost:8050/health` - overall service health status (should be `SERVING` if all is fine)
  - **GET** `http://localhost:8050/api/v1/status/indexers` - status and detailed info for indexers

### Stopping the containers

```bash
docker-compose down
```

Data is persisted in the `./database` volume (relative to the compose file). Remove the folder or run `docker-compose down -v` to start with a clean DB (only if you intend to drop all indexed data).
