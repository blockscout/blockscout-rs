# ENVs — `config/avalanche`

Avalanche ICM/ICTT only: C-Chain plus the NUMINE and Henesys subnets, NUMINE as the bridge's home chain.

Grammar, merge semantics, field reference and traps: [`config/ENVs.md`](../ENVs.md). Here — only this set's variables, with their actual values, in two interchangeable forms per entity: one JSON variable, or one variable per field.

Every block below stands alone. Copy a chain block to add that chain, a bridge block to add that bridge, a contract block to add one contract version, or a single line for a pinpoint change.

| Chain | Name | RPC providers |
| --- | --- | --- |
| `43114` | Avalanche C-Chain | `avalanche` |
| `8021` | NUMINE Mainnet | `glacier` |
| `68414` | Henesys | `msu` |

| Bridge | Name | `type` / `indexer_type` | Contracts |
| --- | --- | --- | --- |
| `2` | Avalanche ICTT | `avalanche_native` / `icm_ictt` | 3 |

## Config files

```bash
INTERCHAIN_INDEXER__CHAINS_CONFIG=config/avalanche/chains.json
INTERCHAIN_INDEXER__BRIDGES_CONFIG=config/avalanche/bridges.json
```

## Chains

### Chain `43114` — Avalanche C-Chain

One variable:

```bash
INTERCHAIN_INDEXER_CHAINS__43114='{"name":"Avalanche C-Chain","icon":"https://images.ctfassets.net/gcj8jwzm6086/5VHupNKwnDYJvqMENeV7iJ/3e4b8ff10b69bfa31e70080a4b142cd0/avalanche-avax-logo.svg","explorer":{"url":"https://subnets.avax.network/c-chain","custom_tx_route":"/tx/{hash}","custom_address_route":"/address/{hash}","custom_token_route":"/token/{hash}"},"rpcs":[{"avalanche":{"url":"https://api.avax.network/ext/bc/C/rpc"}}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_CHAINS__43114__NAME='Avalanche C-Chain'
INTERCHAIN_INDEXER_CHAINS__43114__ICON=https://images.ctfassets.net/gcj8jwzm6086/5VHupNKwnDYJvqMENeV7iJ/3e4b8ff10b69bfa31e70080a4b142cd0/avalanche-avax-logo.svg
INTERCHAIN_INDEXER_CHAINS__43114__EXPLORER__URL=https://subnets.avax.network/c-chain
INTERCHAIN_INDEXER_CHAINS__43114__EXPLORER__CUSTOM_TX_ROUTE='/tx/{hash}'
INTERCHAIN_INDEXER_CHAINS__43114__EXPLORER__CUSTOM_ADDRESS_ROUTE='/address/{hash}'
INTERCHAIN_INDEXER_CHAINS__43114__EXPLORER__CUSTOM_TOKEN_ROUTE='/token/{hash}'
# rpc provider "avalanche"
INTERCHAIN_INDEXER_CHAINS__43114__RPCS__AVALANCHE__URL=https://api.avax.network/ext/bc/C/rpc
```

### Chain `8021` — NUMINE Mainnet

One variable:

```bash
INTERCHAIN_INDEXER_CHAINS__8021='{"name":"NUMINE Mainnet","icon":"https://images.ctfassets.net/gcj8jwzm6086/411JTIUnbER3rI5dpOR54Y/3c0a8e47d58818a66edd868d6a03a135/numine_main_icon.png","explorer":{"url":"https://subnets.avax.network/numi"},"rpcs":[{"glacier":{"url":"https://glacier-api.avax.network/v1/ext/bc/8021/rpc","max_rps":1,"multicall_batching_us":0}}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_CHAINS__8021__NAME='NUMINE Mainnet'
INTERCHAIN_INDEXER_CHAINS__8021__ICON=https://images.ctfassets.net/gcj8jwzm6086/411JTIUnbER3rI5dpOR54Y/3c0a8e47d58818a66edd868d6a03a135/numine_main_icon.png
INTERCHAIN_INDEXER_CHAINS__8021__EXPLORER__URL=https://subnets.avax.network/numi
# rpc provider "glacier" (optionally credentialed — see "RPC provider secrets")
INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__URL=https://glacier-api.avax.network/v1/ext/bc/8021/rpc
INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__MAX_RPS=1
INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__MULTICALL_BATCHING_US=0
```

### Chain `68414` — Henesys

One variable:

```bash
INTERCHAIN_INDEXER_CHAINS__68414='{"name":"Henesys","icon":"https://cdn.routescan.io/cdn/chains/henesys/logo.png","explorer":{"url":"https://68414.snowtrace.io/"},"rpcs":[{"msu":{"url":"https://henesys-rpc.msu.io"}}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_CHAINS__68414__NAME=Henesys
INTERCHAIN_INDEXER_CHAINS__68414__ICON=https://cdn.routescan.io/cdn/chains/henesys/logo.png
INTERCHAIN_INDEXER_CHAINS__68414__EXPLORER__URL=https://68414.snowtrace.io/
# rpc provider "msu"
INTERCHAIN_INDEXER_CHAINS__68414__RPCS__MSU__URL=https://henesys-rpc.msu.io
```

## RPC provider secrets

No provider in `chains.json` declares an `api_key`, so **this set needs no secret to
start**. A credential is opt-in, declared entirely through the environment.

Glacier (chain `8021`) accepts an `x-glacier-api-key` header and rate-limits harder
without one. To use a key, declare the credential's shape *and* supply its value — all
three variables together, since a declared `api_key` whose value is unset, empty or
whitespace-only fails startup by design:

```bash
INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__API_KEY__LOCATION=header
INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__API_KEY__PARAM_NAME=x-glacier-api-key
INTERCHAIN_INDEXER_RPC_API_KEY__8021__GLACIER=<secret>
```

Raising `MAX_RPS` above the file's `1` usually goes with the key. Locally all three go
in `.env` (see [`.env.example`](../../.env.example)) and the service is started with
`just run-dev`.

## Bridges

### Bridge `2` — Avalanche ICTT

One variable — all 3 contracts included, since `contracts` is replaced wholesale:

```bash
INTERCHAIN_INDEXER_BRIDGES__2='{"name":"Avalanche ICTT","type":"avalanche_native","indexer_type":"icm_ictt","enabled":true,"api_url":null,"ui_url":null,"docs_url":null,"process_unknown_chains":false,"home_chain_id":8021,"contracts":[{"chain_id":43114,"address":"0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf","version":1,"started_at_block":42526120},{"chain_id":8021,"address":"0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf","version":1,"started_at_block":4},{"chain_id":68414,"address":"0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf","version":1,"started_at_block":4}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_BRIDGES__2__NAME='Avalanche ICTT'
INTERCHAIN_INDEXER_BRIDGES__2__TYPE=avalanche_native
INTERCHAIN_INDEXER_BRIDGES__2__INDEXER_TYPE=icm_ictt
INTERCHAIN_INDEXER_BRIDGES__2__ENABLED=true
INTERCHAIN_INDEXER_BRIDGES__2__API_URL=null
INTERCHAIN_INDEXER_BRIDGES__2__UI_URL=null
INTERCHAIN_INDEXER_BRIDGES__2__DOCS_URL=null
INTERCHAIN_INDEXER_BRIDGES__2__PROCESS_UNKNOWN_CHAINS=false
INTERCHAIN_INDEXER_BRIDGES__2__HOME_CHAIN_ID=8021
```

#### Contracts of bridge `2`

```bash
# chain 43114, version 1
INTERCHAIN_INDEXER_BRIDGES__2__CONTRACTS__43114__0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf__1__STARTED_AT_BLOCK=42526120
```

```bash
# chain 8021, version 1
INTERCHAIN_INDEXER_BRIDGES__2__CONTRACTS__8021__0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf__1__STARTED_AT_BLOCK=4
```

```bash
# chain 68414, version 1
INTERCHAIN_INDEXER_BRIDGES__2__CONTRACTS__68414__0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf__1__STARTED_AT_BLOCK=4
```

## Variant: `bridges_cut.json`

Same bridge with higher catch-up floors (shorter backfill for local runs) — three variables on top of the set above.

```bash
INTERCHAIN_INDEXER_BRIDGES__2__CONTRACTS__43114__0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf__1__STARTED_AT_BLOCK=88000000
INTERCHAIN_INDEXER_BRIDGES__2__CONTRACTS__8021__0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf__1__STARTED_AT_BLOCK=899998
INTERCHAIN_INDEXER_BRIDGES__2__CONTRACTS__68414__0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf__1__STARTED_AT_BLOCK=16176032
```
