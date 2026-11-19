# ENVs — `config/op-interop`

OP Stack native interop only, in two independent two-chain sets, each its own dependency set and bridge. The indexer reads `L2ToL2CrossDomainMessenger` messages between the two chains of a set, up to the `finalized` block tag by default.

- [Devnet set](#devnet-set): OP Labs `sepolia-devnet-4` (chain `420130019`) and `sepolia-devnet-5` (chain `420130020`), `bridge_id` `2004`. Lagoon is active; a test message was sent there, but it could not be executed on the destination.
- [Demo set](#demo-set): OP Labs `interop-demo-0` (chain `420120232`) and `interop-demo-1` (chain `420120233`), `bridge_id` `2005`. Carries real traffic in both directions.

Load one set per run: the files of each set list only its own chains.

Grammar, merge semantics, field reference and traps: [`config/ENVs.md`](../ENVs.md). Here — only this set's variables, with their actual values, in two interchangeable forms per entity: one JSON variable, or one variable per field.

Every block below stands alone. Copy a chain block to add that chain, a bridge block to add that bridge, a contract block to add one contract entry, or a single line for a pinpoint change.

## Devnet set

| Chain | Name | RPC providers |
| --- | --- | --- |
| `420130019` | OP Sepolia Devnet 4 | `optimism` |
| `420130020` | OP Sepolia Devnet 5 | `optimism` |

| Bridge | Name | `type` / `indexer_type` | Contracts |
| --- | --- | --- | --- |
| `2004` | OP Interop (sepolia-devnet-4/5) | `op_interop` / `op_interop` | 6 |

The endpoints answer HTTP 429 above about three calls per second, hence `max_rps` `2`. Use one RPC backend (or a consistently wired pool) per chain: nodes can disagree about the confirmed head.

### Config files

```bash
INTERCHAIN_INDEXER__CHAINS_CONFIG=config/op-interop/chains-devnet.json
INTERCHAIN_INDEXER__BRIDGES_CONFIG=config/op-interop/bridges-devnet.json
```

`just run-op-devnet` starts the service with exactly these two files.

### Chains

#### Chain `420130019` — OP Sepolia Devnet 4

One variable:

```bash
INTERCHAIN_INDEXER_CHAINS__420130019='{"name":"OP Sepolia Devnet 4","icon":"","rpcs":[{"optimism":{"url":"https://sepolia-devnet-4.optimism.io/rpc","max_rps":2}}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_CHAINS__420130019__NAME='OP Sepolia Devnet 4'
INTERCHAIN_INDEXER_CHAINS__420130019__ICON=''
# rpc provider "optimism"
INTERCHAIN_INDEXER_CHAINS__420130019__RPCS__OPTIMISM__URL=https://sepolia-devnet-4.optimism.io/rpc
INTERCHAIN_INDEXER_CHAINS__420130019__RPCS__OPTIMISM__MAX_RPS=2
```

#### Chain `420130020` — OP Sepolia Devnet 5

One variable:

```bash
INTERCHAIN_INDEXER_CHAINS__420130020='{"name":"OP Sepolia Devnet 5","icon":"","rpcs":[{"optimism":{"url":"https://sepolia-devnet-5.optimism.io/rpc","max_rps":2}}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_CHAINS__420130020__NAME='OP Sepolia Devnet 5'
INTERCHAIN_INDEXER_CHAINS__420130020__ICON=''
# rpc provider "optimism"
INTERCHAIN_INDEXER_CHAINS__420130020__RPCS__OPTIMISM__URL=https://sepolia-devnet-5.optimism.io/rpc
INTERCHAIN_INDEXER_CHAINS__420130020__RPCS__OPTIMISM__MAX_RPS=2
```

### Bridges

#### Bridge `2004` — OP Interop (sepolia-devnet-4/5)

`contracts` is replaced wholesale by the single-variable form, so a per-field
override is the safer pinpoint change once the bridge already exists — use
the field-by-field contract blocks below rather than re-supplying the whole
bridge as JSON.

```bash
INTERCHAIN_INDEXER_BRIDGES__2004__NAME='OP Interop (sepolia-devnet-4/5)'
INTERCHAIN_INDEXER_BRIDGES__2004__TYPE=op_interop
INTERCHAIN_INDEXER_BRIDGES__2004__INDEXER_TYPE=op_interop
INTERCHAIN_INDEXER_BRIDGES__2004__ENABLED=true
INTERCHAIN_INDEXER_BRIDGES__2004__API_URL=null
INTERCHAIN_INDEXER_BRIDGES__2004__UI_URL=null
INTERCHAIN_INDEXER_BRIDGES__2004__DOCS_URL='https://specs.optimism.io/interop/messaging.html'
```

##### Contracts of bridge `2004`

Every chain needs one entry of each `kind`: `cross_l2_inbox`
(`0x4200000000000000000000000000000000000022`), `l2_to_l2_messenger`
(`0x4200000000000000000000000000000000000023`) and `superchain_eth_bridge`
(`0x4200000000000000000000000000000000000024`). The address must be that
predeploy. `version` is this service's grammar revision (`1` = `CrossL2Inbox`
1.1.0 / `L2ToL2CrossDomainMessenger` 2.1.0 / `SuperchainETHBridge` 2.0.0), not
an on-chain counter.
`started_at_block` is the chain's Lagoon activation block. There is no `ABI`
variable: the events are pinned in code.

```bash
# chain 420130019 (devnet 4), CrossL2Inbox
INTERCHAIN_INDEXER_BRIDGES__2004__CONTRACTS__420130019__0x4200000000000000000000000000000000000022__1__STARTED_AT_BLOCK=465679
INTERCHAIN_INDEXER_BRIDGES__2004__CONTRACTS__420130019__0x4200000000000000000000000000000000000022__1__KIND=cross_l2_inbox
```

```bash
# chain 420130019 (devnet 4), L2ToL2CrossDomainMessenger
INTERCHAIN_INDEXER_BRIDGES__2004__CONTRACTS__420130019__0x4200000000000000000000000000000000000023__1__STARTED_AT_BLOCK=465679
INTERCHAIN_INDEXER_BRIDGES__2004__CONTRACTS__420130019__0x4200000000000000000000000000000000000023__1__KIND=l2_to_l2_messenger
```

```bash
# chain 420130019 (devnet 4), SuperchainETHBridge
INTERCHAIN_INDEXER_BRIDGES__2004__CONTRACTS__420130019__0x4200000000000000000000000000000000000024__1__STARTED_AT_BLOCK=465679
INTERCHAIN_INDEXER_BRIDGES__2004__CONTRACTS__420130019__0x4200000000000000000000000000000000000024__1__KIND=superchain_eth_bridge
```

```bash
# chain 420130020 (devnet 5), CrossL2Inbox
INTERCHAIN_INDEXER_BRIDGES__2004__CONTRACTS__420130020__0x4200000000000000000000000000000000000022__1__STARTED_AT_BLOCK=829213
INTERCHAIN_INDEXER_BRIDGES__2004__CONTRACTS__420130020__0x4200000000000000000000000000000000000022__1__KIND=cross_l2_inbox
```

```bash
# chain 420130020 (devnet 5), L2ToL2CrossDomainMessenger
INTERCHAIN_INDEXER_BRIDGES__2004__CONTRACTS__420130020__0x4200000000000000000000000000000000000023__1__STARTED_AT_BLOCK=829213
INTERCHAIN_INDEXER_BRIDGES__2004__CONTRACTS__420130020__0x4200000000000000000000000000000000000023__1__KIND=l2_to_l2_messenger
```

```bash
# chain 420130020 (devnet 5), SuperchainETHBridge
INTERCHAIN_INDEXER_BRIDGES__2004__CONTRACTS__420130020__0x4200000000000000000000000000000000000024__1__STARTED_AT_BLOCK=829213
INTERCHAIN_INDEXER_BRIDGES__2004__CONTRACTS__420130020__0x4200000000000000000000000000000000000024__1__KIND=superchain_eth_bridge
```

## Demo set

| Chain | Name | RPC providers |
| --- | --- | --- |
| `420120232` | OP Interop Demo 0 | `optimism` |
| `420120233` | OP Interop Demo 1 | `optimism` |

| Bridge | Name | `type` / `indexer_type` | Contracts |
| --- | --- | --- | --- |
| `2005` | OP Interop (interop-demo-0/1) | `op_interop` / `op_interop` | 6 |

Both chains share one genesis timestamp; demo 0 has 2 s blocks and demo 1 has
1 s blocks, so the one interop activation time (2026-09-11 17:00:00 UTC) falls
on block `32376` and block `64752` respectively. It is a bundled upgrade block:
most `0x42…` predeploys are upgraded there, and `ETHLiquidity` is funded.

The deployed predeploys report older `version()` strings than the devnet set —
`CrossL2Inbox` 1.0.2, `L2ToL2CrossDomainMessenger` 1.3.1,
`SuperchainETHBridge` 1.0.1 — but their events, message hash, `relayETH`
payload and `ExecutingMessage` payload hash are byte-identical to grammar
`version` `1`. That was checked against every predeploy log on both chains,
so `version` stays `1`.

`eth_getLogs` allows up to 100000 blocks per call, so `just run-op-demo` raises
`BATCH_SIZE` to `10000`; at the default `500` the 5.9M-block catch-up of demo 1
takes ~12k calls at `max_rps` `2`. Use one RPC backend per chain, as above.

### Config files

```bash
INTERCHAIN_INDEXER__CHAINS_CONFIG=config/op-interop/chains-demo.json
INTERCHAIN_INDEXER__BRIDGES_CONFIG=config/op-interop/bridges-demo.json
INTERCHAIN_INDEXER__OP_INTEROP_INDEXER__BATCH_SIZE=10000
```

`just run-op-demo` starts the service with exactly these values.

### Chains

#### Chain `420120232` — OP Interop Demo 0

One variable:

```bash
INTERCHAIN_INDEXER_CHAINS__420120232='{"name":"OP Interop Demo 0","icon":"","rpcs":[{"optimism":{"url":"https://interop-demo-0.optimism.io","max_rps":2}}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_CHAINS__420120232__NAME='OP Interop Demo 0'
INTERCHAIN_INDEXER_CHAINS__420120232__ICON=''
# rpc provider "optimism"
INTERCHAIN_INDEXER_CHAINS__420120232__RPCS__OPTIMISM__URL=https://interop-demo-0.optimism.io
INTERCHAIN_INDEXER_CHAINS__420120232__RPCS__OPTIMISM__MAX_RPS=2
```

#### Chain `420120233` — OP Interop Demo 1

One variable:

```bash
INTERCHAIN_INDEXER_CHAINS__420120233='{"name":"OP Interop Demo 1","icon":"","rpcs":[{"optimism":{"url":"https://interop-demo-1.optimism.io","max_rps":2}}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_CHAINS__420120233__NAME='OP Interop Demo 1'
INTERCHAIN_INDEXER_CHAINS__420120233__ICON=''
# rpc provider "optimism"
INTERCHAIN_INDEXER_CHAINS__420120233__RPCS__OPTIMISM__URL=https://interop-demo-1.optimism.io
INTERCHAIN_INDEXER_CHAINS__420120233__RPCS__OPTIMISM__MAX_RPS=2
```

### Bridges

#### Bridge `2005` — OP Interop (interop-demo-0/1)

As for bridge `2004`, prefer the field-by-field contract blocks for a pinpoint
change once the bridge exists.

```bash
INTERCHAIN_INDEXER_BRIDGES__2005__NAME='OP Interop (interop-demo-0/1)'
INTERCHAIN_INDEXER_BRIDGES__2005__TYPE=op_interop
INTERCHAIN_INDEXER_BRIDGES__2005__INDEXER_TYPE=op_interop
INTERCHAIN_INDEXER_BRIDGES__2005__ENABLED=true
INTERCHAIN_INDEXER_BRIDGES__2005__API_URL=null
INTERCHAIN_INDEXER_BRIDGES__2005__UI_URL=null
INTERCHAIN_INDEXER_BRIDGES__2005__DOCS_URL='https://specs.optimism.io/interop/messaging.html'
```

##### Contracts of bridge `2005`

Same kinds, addresses and `version` as bridge `2004`; `started_at_block` is
the activation block above.

```bash
# chain 420120232 (demo 0), CrossL2Inbox
INTERCHAIN_INDEXER_BRIDGES__2005__CONTRACTS__420120232__0x4200000000000000000000000000000000000022__1__STARTED_AT_BLOCK=32376
INTERCHAIN_INDEXER_BRIDGES__2005__CONTRACTS__420120232__0x4200000000000000000000000000000000000022__1__KIND=cross_l2_inbox
```

```bash
# chain 420120232 (demo 0), L2ToL2CrossDomainMessenger
INTERCHAIN_INDEXER_BRIDGES__2005__CONTRACTS__420120232__0x4200000000000000000000000000000000000023__1__STARTED_AT_BLOCK=32376
INTERCHAIN_INDEXER_BRIDGES__2005__CONTRACTS__420120232__0x4200000000000000000000000000000000000023__1__KIND=l2_to_l2_messenger
```

```bash
# chain 420120232 (demo 0), SuperchainETHBridge
INTERCHAIN_INDEXER_BRIDGES__2005__CONTRACTS__420120232__0x4200000000000000000000000000000000000024__1__STARTED_AT_BLOCK=32376
INTERCHAIN_INDEXER_BRIDGES__2005__CONTRACTS__420120232__0x4200000000000000000000000000000000000024__1__KIND=superchain_eth_bridge
```

```bash
# chain 420120233 (demo 1), CrossL2Inbox
INTERCHAIN_INDEXER_BRIDGES__2005__CONTRACTS__420120233__0x4200000000000000000000000000000000000022__1__STARTED_AT_BLOCK=64752
INTERCHAIN_INDEXER_BRIDGES__2005__CONTRACTS__420120233__0x4200000000000000000000000000000000000022__1__KIND=cross_l2_inbox
```

```bash
# chain 420120233 (demo 1), L2ToL2CrossDomainMessenger
INTERCHAIN_INDEXER_BRIDGES__2005__CONTRACTS__420120233__0x4200000000000000000000000000000000000023__1__STARTED_AT_BLOCK=64752
INTERCHAIN_INDEXER_BRIDGES__2005__CONTRACTS__420120233__0x4200000000000000000000000000000000000023__1__KIND=l2_to_l2_messenger
```

```bash
# chain 420120233 (demo 1), SuperchainETHBridge
INTERCHAIN_INDEXER_BRIDGES__2005__CONTRACTS__420120233__0x4200000000000000000000000000000000000024__1__STARTED_AT_BLOCK=64752
INTERCHAIN_INDEXER_BRIDGES__2005__CONTRACTS__420120233__0x4200000000000000000000000000000000000024__1__KIND=superchain_eth_bridge
```

## Indexer settings

Applies to whichever set is loaded. The values are the defaults.

```bash
INTERCHAIN_INDEXER__OP_INTEROP_INDEXER__PULL_INTERVAL_MS=2000
INTERCHAIN_INDEXER__OP_INTEROP_INDEXER__BATCH_SIZE=500
INTERCHAIN_INDEXER__OP_INTEROP_INDEXER__CONFIRMATION_BLOCK_TAG=finalized
```

`CONFIRMATION_BLOCK_TAG` is `finalized` or `safe`; `latest` is not accepted.
See `README.md`'s `INTERCHAIN_INDEXER__OP_INTEROP_INDEXER__*` block for the
full field reference, including `failure_retry`.
