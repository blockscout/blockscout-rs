Interchain Indexer Service
===

**Interchain Indexer** — a standalone Rust microservice designed to index and aggregate cross-chain interactions across multiple networks.

It extends the Blockscout ecosystem with the ability to process and unify data from bridges, native interop protocols, and other interchain mechanisms.

Traditional Blockscout instances are designed for single-network indexing.  
However, as cross-chain ecosystems evolve, monitoring interactions between multiple chains becomes essential.

`interchain-indexer` provides an independent service that:
- Collects and indexes cross-chain transaction data directly from multiple networks.
- Supports various bridge types and native interop mechanisms via pluggable workers.
- Maintains its own database schema optimized for interchain data representation.
- Can operate without relying on single-chain Blockscout instances.
- Provides a set of endpoints for querying interchain messages and transfers.

## Common Design Principles

1. The service works across multiple networks (a cluster) and can index several bridges.
    - **Maximal variant:** multiple networks with heterogeneous cross-chain mechanisms (e.g., Optimism Superchain with native interop and lock/mint bridges).
    - **Base variant:** two networks connected by one bridge (e.g., Gnosis ↔ Ethereum via OmniBridge).
    - **Minimal variant:** indexing L1 → L2 deposits within a single L2.
2. The service must operate **independently** from Blockscout instances. Integration should be optional and not a dependency.
3. It should focus on **cross-chain interactions**, primarily token transfers, while retaining other metadata for potential future use and schema migrations.
4. Configuration (networks, bridges, contracts) should be stored as JSON in a local database, not as environment variables.
5. Each cross-chain mechanism's indexing logic should be implemented as a **plugin**, not a config parameter, to ensure scalability.
6. Planned workers:
    - **CrosschainIndexer** indexes a single bridge and optionally combines the BridgeContractIndexer and MessageCollector entities.
    - **BridgeContractIndexer** collects raw on-chain events for each bridge contract and stores them in `bridge_txs`.
    - **MessageCollector** processes raw events into structured cross-chain messages and transfers.
    - **TokenFetcher** fetches metadata for newly encountered tokens.
    - **Renderer** serves processed data to users and external consumers.

## Configuration JSON Files

The service reads configuration from two JSON files:

- **Chains** — `INTERCHAIN_INDEXER__CHAINS_CONFIG` (e.g. `config/avalanche/chains.json`)
- **Bridges** — `INTERCHAIN_INDEXER__BRIDGES_CONFIG` (e.g. `config/avalanche/bridges.json`)

### `chains.json`

Defines the blockchains the indexer knows about. Each entry describes one chain:

| Field        | Description |
| ------------ | ----------- |
| `chain_id`   | Numeric chain identifier (e.g. 43114 for Avalanche C-Chain). |
| `name`       | Human-readable chain name. |
| `icon`       | Optional URL to chain icon. |
| `explorer`   | Optional explorer base URL and routes: `url`, `custom_tx_route`, `custom_address_route`, `custom_token_route`. |
| `rpcs`       | RPC config per chain. |

### `bridges.json`

Defines which bridges (cross-chain mechanisms) to index. Each entry is one bridge:

| Field        | Description |
| ------------ | ----------- |
| `bridge_id`  | Unique numeric id for the bridge. |
| `name`       | Human-readable bridge name. |
| `type`       | Bridge type (e.g. `avalanche_native`). |
| `indexer_type` | Indexer implementation (e.g. `icm_ictt`). |
| `enabled`    | Whether this bridge is indexed. |
| `api_url` / `ui_url` / `docs_url` | Optional external links. |
| `process_unknown_chains` | When `true`, allow messages with one unknown endpoint. When `false` (default), both endpoints must be configured chains. |
| `home_chain_id` | Optional chain id that narrows processing to messages where at least one endpoint is this chain. |
| `contracts`  | Per-chain contract config: `chain_id`, `address`, `version`, `started_at_block`, optional `kind`, and optional inline `abi`. AMB uses `kind: "amb_proxy"` and `kind: "omnibridge_mediator"`; Avalanche configs leave `kind` unset. |

`process_unknown_chains` and `home_chain_id` apply as two sequential filters:

| `process_unknown_chains` | `home_chain_id` | Behavior |
| ------------------------ | --------------- | -------- |
| `false` (default) | `None` | Only process messages where both endpoints are configured chains. |
| `false` | `Some(h)` | Both endpoints must be configured and at least one endpoint must be `h`. |
| `true` | `None` | Process messages with at least one configured endpoint. |
| `true` | `Some(h)` | Process messages where at least one endpoint is `h` (unknown chains allowed). |

**`started_at_block`** — indexer starts scanning from this block on associated chain; set it to reduce initial sync time or to start from a specific deployment block.

### Overriding `chains.json` / `bridges.json` via environment

At startup, environment variables under two dedicated prefixes are deep-merged
into the JSON read from the config files, **before** validation. Env always wins
over the file. Both single-field overrides and whole new entries (a new chain,
bridge, RPC provider, or contract version) are supported. With no such vars set,
behavior is unchanged.

Note the single underscore after `INTERCHAIN_INDEXER` — these prefixes are
separate from the main `INTERCHAIN_INDEXER__*` settings:

- `INTERCHAIN_INDEXER_CHAINS…` patches the chains config
- `INTERCHAIN_INDEXER_BRIDGES…` patches the bridges config

**Path grammar** (segments are separated by `__` and are case-insensitive):

```
<PREFIX>                                  = whole-config array patch (value must be a JSON array)
<PREFIX>__<ID>                            = one entry (value: JSON object fragment)
<PREFIX>__<ID>__<FIELD>[__<FIELD>…]       = one field (value: scalar or JSON fragment)
```

**Array addressing** — arrays are addressed by id key(s), aligned with the DB
uniqueness keys, so entries that merge together are exactly the entries that
upsert to the same DB row:

| JSON location | Key | Env key segments |
|---|---|---|
| chains top-level array | `chain_id` | `INTERCHAIN_INDEXER_CHAINS__<CHAIN_ID>` |
| bridges top-level array | `bridge_id` | `INTERCHAIN_INDEXER_BRIDGES__<BRIDGE_ID>` |
| `bridges[].contracts` | `(chain_id, address, version)` | `…__CONTRACTS__<CHAIN_ID>__<ADDRESS>__<VERSION>` |
| `chains[].rpcs` | provider name (map key) | `…__RPCS__<PROVIDER_NAME>` |

Matching is exact: numbers numerically, strings (addresses) case-insensitively.
No match appends a new element with the key fields injected; more than one match
fails startup.

**Values** are parsed as JSON first, falling back to a plain string. So `true`,
`123`, `null`, `{…}`, `[…]` are JSON; `Polygon`, URLs, and `0x…` hex stay
strings. A *literal string* that happens to be valid JSON needs JSON-string
quoting: `NAME='"123"'` sets the string `123`. Beware zero-padded numbers:
`VERSION_FIELD=06` is not valid JSON, becomes the string `"06"`, and fails the
typed parse for numeric fields.

**Merge semantics:**

- Patches apply shallow-first: an entry fragment lands before deeper
  field-level vars, so the more specific var always wins.
- Objects deep-merge recursively; `null` **replaces** a field value but never
  removes the key (`"api_url": null` stays in the JSON).
- `null` for a whole entry (`…_CHAINS__137=null`) is an error — deletion via
  env is not supported.
- A nested whole-array value (`…__RPCS='[…]'`, `…__CONTRACTS='[…]'`) **replaces**
  the array wholesale (escape hatch).
- The bare prefix takes a JSON array; each element must contain the id field
  and is upserted (merged into the matching entry, or appended).
- Missing intermediate containers are created on demand, so a brand-new entry
  can be built entirely from field-level vars.
- Id fields inside an entry fragment (or a direct id-field var like
  `…__137__CHAIN_ID=…`) must match the key the entry is addressed by, or be
  omitted — a conflicting value fails startup instead of silently retargeting
  the entry. Entry values must be JSON objects.
- The merged result goes through the same strict validation as the files —
  unknown fields, missing required fields, or type mismatches fail startup.
  Every applied override is logged at startup (`applied config env override`);
  when an override **replaces an existing value**, the old and new values are
  logged too (`config env override replaced an existing value`). Newly added
  fields/entries are logged without values.

**Examples:**

```bash
# Disable bridge 1
INTERCHAIN_INDEXER_BRIDGES__1__ENABLED=false

# Null out an optional field (key is kept, value becomes null)
INTERCHAIN_INDEXER_BRIDGES__1__API_URL=null

# Add a new chain field-by-field
INTERCHAIN_INDEXER_CHAINS__137__NAME=Polygon
INTERCHAIN_INDEXER_CHAINS__137__ICON=https://example.com/polygon.svg
INTERCHAIN_INDEXER_CHAINS__137__RPCS__MYNODE__URL=https://my.polygon.node

# …or as one JSON fragment (chain_id is injected from the path)
INTERCHAIN_INDEXER_CHAINS__137='{"name":"Polygon","icon":"https://example.com/polygon.svg","rpcs":[{"mynode":{"url":"https://my.polygon.node"}}]}'

# Tune an existing RPC provider / add a new one
INTERCHAIN_INDEXER_CHAINS__1__RPCS__DRPC__MAX_RPS=5
INTERCHAIN_INDEXER_CHAINS__1__RPCS__MYNODE='{"url":"https://my.eth.node","max_rps":2}'

# Tune an existing contract by (chain_id, address, version)…
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d__6__STARTED_AT_BLOCK=18588922

# …or add a new contract *version* for the same chain+address (appends a new entry)
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d__8__STARTED_AT_BLOCK=19000000
```

## Envs

### Main Service Settings

[anchor]: <> (anchors.envs.start.service)

| Variable                                                                | Req&#x200B;uir&#x200B;ed | Description                                                                                                                                                                                                                                                                                                                                                                                                                                                                 | Default value |
| ----------------------------------------------------------------------- | ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------- |
| `INTERCHAIN_INDEXER__BRIDGES_CONFIG`                                    | true                     | e.g. `config/avalanche/bridges.json`                                                                                                                                                                                                                                                                                                                                                                                                                                        |               |
| `INTERCHAIN_INDEXER__CHAINS_CONFIG`                                     | true                     | e.g. `config/avalanche/chains.json`                                                                                                                                                                                                                                                                                                                                                                                                                                         |               |
| `INTERCHAIN_INDEXER__DATABASE__CONNECT__URL`                            | true                     | e.g. `postgres://postgres:postgres@database:5433/blockscout`                                                                                                                                                                                                                                                                                                                                                                                                                |               |
| `INTERCHAIN_INDEXER__DATABASE__CREATE_DATABASE`                         |                          | Create database on service startup                                                                                                                                                                                                                                                                                                                                                                                                                                          | `false`       |
| `INTERCHAIN_INDEXER__DATABASE__RUN_MIGRATIONS`                          |                          | Run DB migrations on startup                                                                                                                                                                                                                                                                                                                                                                                                                                                | `false`       |
| `INTERCHAIN_INDEXER__API__DEFAULT_PAGE_SIZE`                            |                          | Default page size for paginated endpoints (`/api/v1/interchain/messages` and `/api/v1/interchain/transfers`)                                                                                                                                                                                                                                                                                                                                                                | `50`          |
| `INTERCHAIN_INDEXER__API__MAX_PAGE_SIZE`                                |                          | Maximum supported page size for paginated endpoints (configured via `page_size` query parameter)                                                                                                                                                                                                                                                                                                                                                                            | `100`         |
| `INTERCHAIN_INDEXER__API__USE_PAGINATION_TOKEN`                         |                          | If true, wrap all raw pagination parameters into the single Base64 string                                                                                                                                                                                                                                                                                                                                                                                                   | `true`        |
| `INTERCHAIN_INDEXER__TOKEN_INFO__BLOCKSCOUT_TOKEN_INFO__IGNORE_CHAINS`  |                          | The list of chain IDs to be ignored by token info service. Comma-separated list of identifiers without spaces (e.g. `42,1000`)                                                                                                                                                                                                                                                                                                                                              | ``            |
| `INTERCHAIN_INDEXER__TOKEN_INFO__BLOCKSCOUT_TOKEN_INFO__RETRY_INTERVAL` |                          | If the token icon is not found in the external token info service do not retry fetching it during this interval. Unit: `seconds`                                                                                                                                                                                                                                                                                                                                            | `3600`        |
| `INTERCHAIN_INDEXER__TOKEN_INFO__BLOCKSCOUT_TOKEN_INFO__URL`            |                          | External Blockscout token info service. E.g. `https://contracts-info-test.k8s-dev.blockscout.com`                                                                                                                                                                                                                                                                                                                                                                           | `null`        |
| `INTERCHAIN_INDEXER__TOKEN_INFO__ONCHAIN_RETRY_INTERVAL`                |                          | If the on-chain request for the token info was unsuccessful, do not retry fetching it during this interval. Unit: `seconds`                                                                                                                                                                                                                                                                                                                                                 | `10`          |
| `INTERCHAIN_INDEXER__CHAIN_INFO__COOLDOWN_INTERVAL`                     |                          | If the chain name is unknown, do not retry DB query during this interval. Unit: `seconds`                                                                                                                                                                                                                                                                                                                                                                                   | `60`          |
| `INTERCHAIN_INDEXER__BUFFER_SETTINGS__HOT_TTL`                          |                          |                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `10`          |
| `INTERCHAIN_INDEXER__BUFFER_SETTINGS__MAINTENANCE_INTERVAL`             |                          |                                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `500`         |
| `INTERCHAIN_INDEXER__STATS__BACKFILL_ON_START`                          |                          | Recalculate the statistics tables for messages and transfers (`stats_messages`, `stats_asset*`) on service startup. This is needed only after the first application of the `m20260312_175120_add_stats_tables` migration, and only if there are existing DB records before it. This option should normally be disabled after the migration to reduce service startup time.                                                                                                  | `false`       |
| `INTERCHAIN_INDEXER__STATS__CHAINS_RECALCULATION_PERIOD_SECS`           |                          | Interval in seconds between full recomputations of per-chain distinct user counters in `stats_chains` (from `crosschain_messages` / `crosschain_transfers`, any status). Only chains with at least one counted user address keep a row; stale rows are deleted. Set to `0` to disable the background task.                                                                                                                                                                  | `3600`        |
| `INTERCHAIN_INDEXER__STATS__INCLUDE_ZERO_CHAINS`                        |                          | When `true`, stats endpoints (`/api/v1/stats/chains` and `/api/v1/stats/chain/{chain_id}/messages-paths/*`) include known chains from `chains` even when the aggregated stats row is missing or has a zero value. For message paths with `counterparty_chain_ids`, zero rows are still returned for the explicitly requested counterparties that exist in `chains`, and no other counterparties are added. Disable it to return only chains with positive aggregated stats. | `true`        |

[anchor]: <> (anchors.envs.end.service)

### Avalanche Indexer Settings

[anchor]: <> (anchors.envs.start.avalanche)

| Variable                                                                   | Req&#x200B;uir&#x200B;ed | Description                                                            | Default value |
| -------------------------------------------------------------------------- | ------------------------ | ---------------------------------------------------------------------- | ------------- |
| `INTERCHAIN_INDEXER__AVALANCHE_INDEXER__BATCH_SIZE`                        |                          | Number of contract events to be pulled at once.                        | `1000`        |
| `INTERCHAIN_INDEXER__AVALANCHE_INDEXER__PULL_INTERVAL_MS`                  |                          | Duration between pulling contract events. Unit: `milliseconds`         | `10000`       |
| `INTERCHAIN_INDEXER__AVALANCHE_INDEXER__DATA_API_CLIENT_SETTINGS__NETWORK` |                          | Avalanche Data API network. One of `mainnet`, `fuji`, `testnet`.       | `Mainnet`     |
| `INTERCHAIN_INDEXER__AVALANCHE_INDEXER__DATA_API_CLIENT_SETTINGS__API_KEY` |                          | API key for Avalanche Data API (`x-glacier-api-key` header). Optional. | `null`        |

[anchor]: <> (anchors.envs.end.avalanche)

### AMB Indexer Settings

[anchor]: <> (anchors.envs.start.amb)

| Variable                                                     | Req&#x200B;uir&#x200B;ed | Description                                                    | Default value |
| ------------------------------------------------------------ | ------------------------ | -------------------------------------------------------------- | ------------- |
| `INTERCHAIN_INDEXER__AMB_INDEXER__BATCH_SIZE`                |                          | Number of contract events to be pulled at once.                | `1000`        |
| `INTERCHAIN_INDEXER__AMB_INDEXER__PULL_INTERVAL_MS`          |                          | Duration between pulling contract events. Unit: `milliseconds` | `500`       |
| `INTERCHAIN_INDEXER__AMB_INDEXER__RECEIPT_CONCURRENCY`       |                          | Maximum concurrent receipt and block fetches per AMB batch.    | `25`          |
| `INTERCHAIN_INDEXER__AMB_INDEXER__CLOCK_SKEW_TOLERANCE`      |                          | Tolerance for a destination execution preceding its source request before flagging an AMB `messageId` collision. Unit: `seconds` | `300`         |

[anchor]: <> (anchors.envs.end.amb)

### Metrics Settings (Prometheus-compatible)

[anchor]: <> (anchors.envs.start.metrics)

| Variable                               | Req&#x200B;uir&#x200B;ed | Description | Default value  |
| -------------------------------------- | ------------------------ | ----------- | -------------- |
| `INTERCHAIN_INDEXER__METRICS__ENABLED` |                          | Enable the metrics server | `false`        |
| `INTERCHAIN_INDEXER__METRICS__ADDR`    |                          | Address for the metrics listener | `0.0.0.0:6060` |
| `INTERCHAIN_INDEXER__METRICS__ROUTE`   |                          | HTTP path for metrics scraping | `/metrics`     |

[anchor]: <> (anchors.envs.end.metrics)

Expose the metrics port (default `6060`) when running in Docker (see docker-compose.yml) and scrape `{addr}{route}`.

### Auxiliary Settings

<details><summary>Server settings</summary>
<p>

[anchor]: <> (anchors.envs.start.server)

| Variable                                                           | Req&#x200B;uir&#x200B;ed | Description | Default value                            |
| ------------------------------------------------------------------ | ------------------------ | ----------- | ---------------------------------------- |
| `INTERCHAIN_INDEXER__SERVER__GRPC__ADDR`                           |                          |             | `0.0.0.0:8051`                           |
| `INTERCHAIN_INDEXER__SERVER__GRPC__ENABLED`                        |                          |             | `false`                                  |
| `INTERCHAIN_INDEXER__SERVER__HTTP__ADDR`                           |                          |             | `0.0.0.0:8050`                           |
| `INTERCHAIN_INDEXER__SERVER__HTTP__BASE_PATH`                      |                          |             | `null`                                   |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__ALLOWED_CREDENTIALS`      |                          |             | `true`                                   |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__ALLOWED_METHODS`          |                          |             | `PUT, GET, POST, OPTIONS, DELETE, PATCH` |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__ALLOWED_ORIGIN`           |                          |             | ``                                       |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__BLOCK_ON_ORIGIN_MISMATCH` |                          |             | `false`                                  |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__ENABLED`                  |                          |             | `false`                                  |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__MAX_AGE`                  |                          |             | `3600`                                   |
| `INTERCHAIN_INDEXER__SERVER__HTTP__CORS__SEND_WILDCARD`            |                          |             | `false`                                  |
| `INTERCHAIN_INDEXER__SERVER__HTTP__ENABLED`                        |                          |             | `true`                                   |
| `INTERCHAIN_INDEXER__SERVER__HTTP__MAX_BODY_SIZE`                  |                          |             | `2097152`                                |

[anchor]: <> (anchors.envs.end.server)

</p>
</details>

<details><summary>Tracing settings</summary>
<p>

[anchor]: <> (anchors.envs.start.tracing)

| Variable                                     | Req&#x200B;uir&#x200B;ed | Description | Default value    |
| -------------------------------------------- | ------------------------ | ----------- | ---------------- |
| `INTERCHAIN_INDEXER__JAEGER__AGENT_ENDPOINT` |                          |             | `127.0.0.1:6831` |
| `INTERCHAIN_INDEXER__JAEGER__ENABLED`        |                          |             | `false`          |
| `INTERCHAIN_INDEXER__TRACING__ENABLED`       |                          |             | `true`           |
| `INTERCHAIN_INDEXER__TRACING__FORMAT`        |                          |             | `default`        |

[anchor]: <> (anchors.envs.end.tracing)

</p>
</details>

## Dev

+ Install [just](https://github.com/casey/just) cli. Just is like make but better.
+ Install [dotenv-cli](https://www.npmjs.com/package/dotenv-cli)
+ Execute `just` to see available dev commands

```bash
just
```
+ Start dev postgres service by just typing

```bash
just start-postgres
```

+ For ORM codegen and migrations install [sea-orm-cli](https://www.sea-ql.org/SeaORM/docs/generate-entity/sea-orm-cli/)


+ Write initial migration inside `interchain-indexer-logic/migration/src/m20220101_000001_create_table`.
+ If you want you can create another migration by just typing:

```bash
just new-migration <name>
```
+ Apply migration by just typing:
    ```bash
    just migrate-up
    ```

+ Generate ORM codegen by just typing:

    ```bash
    just generate-entities
    ```
+ Now you ready to start API server! Just run it:
    ```
    just run
    ```
or run with ENVs from .env current
    ```
    just run-dev
    ```

## Troubleshooting

1. Invalid tonic version

```
`Router` and `Router` have similar names, but are actually distinct types
```

To fix this error you need to change tonic version of `tonic` in `blockscout-service-launcher` to `0.8`

For now you can only change in `Cargo.lock`