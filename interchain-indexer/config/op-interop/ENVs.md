# ENVs — `config/op-interop`

OP Stack native interop only, in one devnet set: OP Labs `sepolia-devnet-4` (chain `420130019`) and `sepolia-devnet-5` (chain `420130020`), one dependency set, `bridge_id` `2004`. The indexer reads `L2ToL2CrossDomainMessenger` messages between the two chains, up to the `finalized` block tag by default.

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
