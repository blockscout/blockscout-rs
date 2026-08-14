# ENVs — `config` (empty base set)

`config/chains.json` and `config/bridges.json` are both `[]` — the neutral base for a **file-less deployment**. Nothing is indexed until environment variables supply entries, and everything they supply is created from scratch rather than merged into existing values.

This file doubles as the shared reference for the whole `config/` tree: grammar, the complete field catalogue, and the traps. Each populated set has its own document listing that set's parameters verbatim:

| Set | Contents | Document |
| --- | --- | --- |
| `config/full-mainnet` | AMB/Omnibridge + Avalanche ICTT, mainnet | [ENVs.md](full-mainnet/ENVs.md) |
| `config/full-testnet` | AMB/Omnibridge, testnet | [ENVs.md](full-testnet/ENVs.md) |
| `config/omnibridge` | AMB/Omnibridge only, mainnet + testnet | [ENVs.md](omnibridge/ENVs.md) |
| `config/avalanche` | Avalanche ICM/ICTT only | [ENVs.md](avalanche/ENVs.md) |

## Prefixes and grammar

| Prefix | Patches | Note |
| --- | --- | --- |
| `INTERCHAIN_INDEXER__…` | the service settings | **double** underscore |
| `INTERCHAIN_INDEXER_CHAINS…` | the chains config | single underscore |
| `INTERCHAIN_INDEXER_BRIDGES…` | the bridges config | single underscore |
| `INTERCHAIN_INDEXER_RPC_API_KEY__…` | RPC provider secrets | single underscore |

The single underscore is load-bearing: the main settings source reads
`INTERCHAIN_INDEXER__*`, so these three families are invisible to it and cannot
collide.

```text
<PREFIX>                                  = whole-config array patch (value: JSON array)
<PREFIX>__<ID>                            = one entry              (value: JSON object)
<PREFIX>__<ID>__<FIELD>[__<FIELD>…]       = one field              (value: scalar or JSON)
```

Segments are separated by `__` and are case-insensitive. Array elements are
addressed by the same keys the database is unique on:

| JSON location | Key | Env key segments |
| --- | --- | --- |
| chains top-level array | `chain_id` | `INTERCHAIN_INDEXER_CHAINS__<CHAIN_ID>` |
| bridges top-level array | `bridge_id` | `INTERCHAIN_INDEXER_BRIDGES__<BRIDGE_ID>` |
| `bridges[].contracts` | `(chain_id, address, version)` | `…__CONTRACTS__<CHAIN_ID>__<ADDRESS>__<VERSION>` |
| `chains[].rpcs` | provider name (map key) | `…__RPCS__<PROVIDER_NAME>` |

No match appends a new element with the key fields injected; more than one match
fails startup. Env is merged **on top of** the file and always wins.

Full narrative: [README — Overriding `chains.json` / `bridges.json` via
environment](../README.md#overriding-chainsjson--bridgesjson-via-environment).

## Which files the service reads

```bash
INTERCHAIN_INDEXER__CHAINS_CONFIG=config/chains.json    # []
INTERCHAIN_INDEXER__BRIDGES_CONFIG=config/bridges.json  # []
```

Both are **required** — the service has no fallback path and fails to start without them. Pointing them at the empty arrays is what turns the variables below from overrides into the entire configuration.

## Short form — JSON values

One variable per entity. `chain_id` / `bridge_id` come from the path and are injected, so the fragment omits them. Missing containers are created on demand, which is what makes a from-scratch entry possible on top of an empty array.

```bash
# a chain
INTERCHAIN_INDEXER_CHAINS__137='{"name":"Polygon","icon":"https://example.com/polygon.svg","explorer":{"url":"https://polygon.blockscout.com"},"rpcs":[{"mynode":{"order":0,"url":"https://my.polygon.node","max_rps":20}}]}'

# a bridge, with its contracts
INTERCHAIN_INDEXER_BRIDGES__1='{"name":"My Bridge","type":"amb","indexer_type":"amb","enabled":true,"api_url":null,"ui_url":null,"docs_url":null,"contracts":[{"chain_id":137,"address":"0x0000000000000000000000000000000000000001","version":6,"started_at_block":55000000,"kind":"amb_proxy"}]}'
```

Or the whole config in one variable — the bare prefix takes a JSON **array**, and each element must carry its id field:

```bash
INTERCHAIN_INDEXER_CHAINS='[{"chain_id":137,"name":"Polygon","icon":"https://example.com/polygon.svg","explorer":{"url":"https://polygon.blockscout.com"},"rpcs":[{"mynode":{"order":0,"url":"https://my.polygon.node","max_rps":20}}]}]'
INTERCHAIN_INDEXER_BRIDGES='[{"bridge_id":1,"name":"My Bridge","type":"amb","indexer_type":"amb","enabled":true,"api_url":null,"ui_url":null,"docs_url":null,"contracts":[{"chain_id":137,"address":"0x0000000000000000000000000000000000000001","version":6,"started_at_block":55000000,"kind":"amb_proxy"}]}]'
```

## Long form — one variable per field

The same two entities, field by field. Every variable below is independent: a deployment template can expose just the ones it needs and leave the rest to the defaults in the reference.

```bash
INTERCHAIN_INDEXER_CHAINS__137__NAME=Polygon
INTERCHAIN_INDEXER_CHAINS__137__ICON=https://example.com/polygon.svg
INTERCHAIN_INDEXER_CHAINS__137__EXPLORER__URL=https://polygon.blockscout.com
INTERCHAIN_INDEXER_CHAINS__137__RPCS__MYNODE__ORDER=0
INTERCHAIN_INDEXER_CHAINS__137__RPCS__MYNODE__URL=https://my.polygon.node
INTERCHAIN_INDEXER_CHAINS__137__RPCS__MYNODE__MAX_RPS=20
```

```bash
INTERCHAIN_INDEXER_BRIDGES__1__NAME='My Bridge'
INTERCHAIN_INDEXER_BRIDGES__1__TYPE=amb
INTERCHAIN_INDEXER_BRIDGES__1__INDEXER_TYPE=amb
INTERCHAIN_INDEXER_BRIDGES__1__ENABLED=true
INTERCHAIN_INDEXER_BRIDGES__1__API_URL=null
INTERCHAIN_INDEXER_BRIDGES__1__UI_URL=null
INTERCHAIN_INDEXER_BRIDGES__1__DOCS_URL=null
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__137__0x0000000000000000000000000000000000000001__6__STARTED_AT_BLOCK=55000000
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__137__0x0000000000000000000000000000000000000001__6__KIND=amb_proxy
```

A bridge contract of an AMB-type bridge also needs its `abi` — see the populated sets for real values, e.g. [`config/full-mainnet/ENVs.md`](full-mainnet/ENVs.md#bridges).

## Field reference

### `chains[]`

| Field | Env path suffix | Required | Default |
| --- | --- | --- | --- |
| `chain_id` | *(taken from the path)* | yes | — |
| `name` | `__NAME` | yes | — |
| `icon` | `__ICON` | yes | — |
| `explorer.url` | `__EXPLORER__URL` | no | `""` |
| `explorer.custom_tx_route` | `__EXPLORER__CUSTOM_TX_ROUTE` | no | `null` |
| `explorer.custom_address_route` | `__EXPLORER__CUSTOM_ADDRESS_ROUTE` | no | `null` |
| `explorer.custom_token_route` | `__EXPLORER__CUSTOM_TOKEN_ROUTE` | no | `null` |
| `pool_config.health_period` (ms) | `__POOL_CONFIG__HEALTH_PERIOD` | no | `1000` |
| `pool_config.max_block_lag` | `__POOL_CONFIG__MAX_BLOCK_LAG` | no | `100` |
| `pool_config.retry_count` | `__POOL_CONFIG__RETRY_COUNT` | no | `5` |
| `pool_config.retry_initial_delay_ms` | `__POOL_CONFIG__RETRY_INITIAL_DELAY_MS` | no | `200` |
| `pool_config.retry_max_delay_ms` | `__POOL_CONFIG__RETRY_MAX_DELAY_MS` | no | `5000` |
| `rpcs` | `__RPCS__<PROVIDER>__…` | yes | — |

### `chains[].rpcs[<provider>]`

| Field | Env path suffix | Required | Default |
| --- | --- | --- | --- |
| `url` | `__URL` | yes | — |
| `enabled` | `__ENABLED` | no | `true` |
| `order` | `__ORDER` | no | unset → ranks after every provider that has one |
| `max_rps` | `__MAX_RPS` | no | `10` |
| `error_threshold` | `__ERROR_THRESHOLD` | no | `3` |
| `cooldown_threshold` | `__COOLDOWN_THRESHOLD` | no | `1` |
| `cooldown_secs` | `__COOLDOWN_SECS` | no | `60` |
| `multicall_batching_us` | `__MULTICALL_BATCHING_US` | no | `60` |
| `api_key` | `__API_KEY__…` | no | `null` |

Provider order inside the pool (element 0 is the startup primary): `order`
ascending, then position of the containing object in the `rpcs` array, then
provider name alphabetically. Key order *inside* one `rpcs` object does not
survive loading — `order` is the only way to express intent.

### `chains[].rpcs[<provider>].api_key`

| Field | Env path suffix | Required | Notes |
| --- | --- | --- | --- |
| `location` | `__API_KEY__LOCATION` | yes | `header` \| `query` \| `path` |
| `param_name` | `__API_KEY__PARAM_NAME` | yes | header name, query parameter name, or `:placeholder` name in `url` |
| `prefix` | `__API_KEY__PREFIX` | no | `header` only (e.g. `Bearer`); rejected for `query`/`path` |
| `value_env` | `__API_KEY__VALUE_ENV` | no | name of the variable holding the secret; default is the derived name |

Derived secret variable: `INTERCHAIN_INDEXER_RPC_API_KEY__<CHAIN_ID>__<PROVIDER>`,
provider uppercased with every character outside `[A-Z0-9_]` replaced by `_`.

| `location` | Key ends up in | `prefix` |
| --- | --- | --- |
| `header` | a request header; the URL is left byte-identical | supported |
| `query` | the URL, as `?<param_name>=<key>` | rejected |
| `path` | the URL, replacing `:<param_name>` | rejected |

`header` is preferred: `query`/`path` keys live in the URL, which third-party
code (e.g. `alloy-transport-http`'s debug span) may render.

### `bridges[]`

| Field | Env path suffix | Required | Default |
| --- | --- | --- | --- |
| `bridge_id` | *(taken from the path)* | yes | — |
| `name` | `__NAME` | yes | — |
| `type` | `__TYPE` | yes | — (`amb`, `avalanche_native`, …) |
| `indexer_type` | `__INDEXER_TYPE` | no | `unknown` (`icm_ictt` \| `amb`) |
| `enabled` | `__ENABLED` | yes | — |
| `api_url` | `__API_URL` | yes (may be `null`) | — |
| `ui_url` | `__UI_URL` | yes (may be `null`) | — |
| `docs_url` | `__DOCS_URL` | yes (may be `null`) | — |
| `process_unknown_chains` | `__PROCESS_UNKNOWN_CHAINS` | no | `false` |
| `home_chain_id` | `__HOME_CHAIN_ID` | no | `null` |
| `reconstruct_incoming_ictt_transfers` | `__RECONSTRUCT_INCOMING_ICTT_TRANSFERS` | no | `true` (Avalanche only) |
| `contracts` | `__CONTRACTS__<CHAIN_ID>__<ADDRESS>__<VERSION>__…` | yes | — |

`indexer_type` selects the indexer; leaving it unset means `unknown` and the
bridge is not indexed at all. `home_chain_id`, when set, must be one of the
chains present in `contracts` — validated at startup.

### `bridges[].contracts[]`

| Field | Env path suffix | Required | Notes |
| --- | --- | --- | --- |
| `chain_id` | *(taken from the path)* | yes | — |
| `address` | *(taken from the path)* | yes | matched case-insensitively |
| `version` | *(taken from the path)* | yes | — |
| `started_at_block` | `__STARTED_AT_BLOCK` | yes | must be ≥ `1`; `0` fails startup |
| `kind` | `__KIND` | no | `amb_proxy`, `omnibridge_mediator`, … |
| `abi` | `__ABI` | no | inline JSON array, or a JSON-quoted string |

## Gotchas

- **`null` replaces, never removes.** `…__API_URL=null` yields `"api_url": null`
  in the merged JSON — the key stays. That is deliberate: several fields are
  `Option` without `#[serde(default)]`, so removing the key would fail the typed
  parse with `missing field`.
- **Deletion is not supported.** `…_CHAINS__137=null` is a startup error. To take
  a chain or bridge out of the picture, disable it (`…__ENABLED=false` for a
  bridge, `…__RPCS__<PROVIDER>__ENABLED=false` for one provider), or ship a
  different file.
- **Values are parsed as JSON first.** `true`, `123`, `null`, `{…}`, `[…]` are
  JSON; `Ethereum`, URLs and `0x…` hex fall back to plain strings. A literal
  string that *is* valid JSON needs JSON-string quoting: `…__NAME='"123"'`.
- **Zero-padded numbers bite.** `…__VERSION=06` is not valid JSON, becomes the
  string `"06"`, and then fails the typed parse of a numeric field.
- **Nested arrays are replaced wholesale.** `…__RPCS='[…]'` and
  `…__CONTRACTS='[…]'` overwrite the array instead of merging into it. Per-entry
  paths (`…__RPCS__DRPC__…`) merge.
- **Shallow patches land first.** An entry fragment is applied before the
  field-level variables addressing the same entry, so the more specific variable
  always wins. Two variables resolving to the *same* path is a startup error.
- **Id fields must agree with the path.** `…__137__CHAIN_ID=1` fails rather than
  silently retargeting the entry. Omitting them is the normal case — they are
  injected from the path.
- **Unknown fields fail startup.** The merged JSON goes through the same
  `deny_unknown_fields` parse as the files, so a typo'd path segment surfaces as
  a config error instead of being ignored.
- **`started_at_block` must be ≥ 1.** A completed catch-up persists
  `started_at_block - 1`, which cannot represent the empty interval below block
  zero.
- **Overrides are logged, values are not.** Every applied override logs
  `applied config env override` with its path at startup, and replacements log
  a second line. Raw values stay at `debug` level — RPC URLs may embed API keys.
