# ENVs — `config/xdai`

xDai bridge only, in two sets: **mainnet** (Ethereum ↔ Gnosis, `bridge_id` `3`) and **testnet** (Sepolia ↔ Chiado, `bridge_id` `1003`). The mainnet pair uses the same two chains as `config/omnibridge` with only the xDai bridge's four contract versions; the testnet pair is the xDai-only subset of `config/full-testnet`, and its chains file is identical to `config/omnibridge/chains-testnet.json`.

Grammar, merge semantics, field reference and traps: [`config/ENVs.md`](../ENVs.md). Here — only these sets' variables, with their actual values, in two interchangeable forms per entity: one JSON variable, or one variable per field.

Every block below stands alone. Copy a chain block to add that chain, a bridge block to add that bridge, a contract block to add one contract version, or a single line for a pinpoint change.

## Mainnet set

| Chain | Name | RPC providers |
| --- | --- | --- |
| `1` | Ethereum | `gateway`, `drpc`, `1rpc` |
| `100` | Gnosis | `gateway`, `gnosis_official` |

| Bridge | Name | `type` / `indexer_type` | Contracts |
| --- | --- | --- | --- |
| `3` | xDai Bridge | `xdai` / `xdai` | 4 |

### Config files

```bash
INTERCHAIN_INDEXER__CHAINS_CONFIG=config/xdai/chains.json
INTERCHAIN_INDEXER__BRIDGES_CONFIG=config/xdai/bridges.json
```

### Chains

#### Chain `1` — Ethereum

One variable:

```bash
INTERCHAIN_INDEXER_CHAINS__1='{"name":"Ethereum","icon":"https://blockscout-icons.s3.us-east-1.amazonaws.com/ethereum.svg","explorer":{"url":"https://eth.blockscout.com","custom_tx_route":"/tx/{hash}","custom_address_route":"/address/{hash}","custom_token_route":"/token/{hash}"},"rpcs":[{"gateway":{"url":"https://rpc.eth.gateway.fm"},"drpc":{"url":"https://eth.drpc.org"},"1rpc":{"url":"https://1rpc.io/eth"}}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_CHAINS__1__NAME=Ethereum
INTERCHAIN_INDEXER_CHAINS__1__ICON=https://blockscout-icons.s3.us-east-1.amazonaws.com/ethereum.svg
INTERCHAIN_INDEXER_CHAINS__1__EXPLORER__URL=https://eth.blockscout.com
INTERCHAIN_INDEXER_CHAINS__1__EXPLORER__CUSTOM_TX_ROUTE='/tx/{hash}'
INTERCHAIN_INDEXER_CHAINS__1__EXPLORER__CUSTOM_ADDRESS_ROUTE='/address/{hash}'
INTERCHAIN_INDEXER_CHAINS__1__EXPLORER__CUSTOM_TOKEN_ROUTE='/token/{hash}'
# rpc provider "gateway"
INTERCHAIN_INDEXER_CHAINS__1__RPCS__GATEWAY__URL=https://rpc.eth.gateway.fm
# rpc provider "drpc"
INTERCHAIN_INDEXER_CHAINS__1__RPCS__DRPC__URL=https://eth.drpc.org
# rpc provider "1rpc"
INTERCHAIN_INDEXER_CHAINS__1__RPCS__1RPC__URL=https://1rpc.io/eth
```

#### Chain `100` — Gnosis

One variable:

```bash
INTERCHAIN_INDEXER_CHAINS__100='{"name":"Gnosis","icon":"https://blockscout-icons.s3.us-east-1.amazonaws.com/gnosis.svg","explorer":{"url":"https://gnosis.blockscout.com","custom_tx_route":"/tx/{hash}","custom_address_route":"/address/{hash}","custom_token_route":"/token/{hash}"},"rpcs":[{"gateway":{"url":"https://rpc.gnosis.gateway.fm"},"gnosis_official":{"url":"https://rpc.gnosischain.com","max_rps":2}}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_CHAINS__100__NAME=Gnosis
INTERCHAIN_INDEXER_CHAINS__100__ICON=https://blockscout-icons.s3.us-east-1.amazonaws.com/gnosis.svg
INTERCHAIN_INDEXER_CHAINS__100__EXPLORER__URL=https://gnosis.blockscout.com
INTERCHAIN_INDEXER_CHAINS__100__EXPLORER__CUSTOM_TX_ROUTE='/tx/{hash}'
INTERCHAIN_INDEXER_CHAINS__100__EXPLORER__CUSTOM_ADDRESS_ROUTE='/address/{hash}'
INTERCHAIN_INDEXER_CHAINS__100__EXPLORER__CUSTOM_TOKEN_ROUTE='/token/{hash}'
# rpc provider "gateway"
INTERCHAIN_INDEXER_CHAINS__100__RPCS__GATEWAY__URL=https://rpc.gnosis.gateway.fm
# rpc provider "gnosis_official"
INTERCHAIN_INDEXER_CHAINS__100__RPCS__GNOSIS_OFFICIAL__URL=https://rpc.gnosischain.com
INTERCHAIN_INDEXER_CHAINS__100__RPCS__GNOSIS_OFFICIAL__MAX_RPS=2
```

### Bridges

#### Bridge `3` — xDai Bridge

`contracts` is replaced wholesale by the single-variable form, so a per-field
override is the safer pinpoint change once the bridge already exists — use
the field-by-field contract blocks below rather than re-supplying the whole
bridge as JSON.

```bash
INTERCHAIN_INDEXER_BRIDGES__3__NAME='xDai Bridge'
INTERCHAIN_INDEXER_BRIDGES__3__TYPE=xdai
INTERCHAIN_INDEXER_BRIDGES__3__INDEXER_TYPE=xdai
INTERCHAIN_INDEXER_BRIDGES__3__ENABLED=true
INTERCHAIN_INDEXER_BRIDGES__3__API_URL=null
INTERCHAIN_INDEXER_BRIDGES__3__UI_URL='https://bridge.gnosischain.com/bridge-explorer/transaction/{{message_id}}'
INTERCHAIN_INDEXER_BRIDGES__3__DOCS_URL='https://docs.gnosischain.com/bridges/About%20Token%20Bridges/xdai-bridge'
```

##### Contracts of bridge `3`

`kind` is deliberately absent for every xDai contract — unlike AMB, xDai has
one contract kind per chain, so the side (Foreign/Home) is inferred from the
ABI's event set, not from a config field.

```bash
# chain 1 (Ethereum, Foreign), version 9 -- epoch floor
INTERCHAIN_INDEXER_BRIDGES__3__CONTRACTS__1__0x4aa42145Aa6Ebf72e164C9bBC74fbD3788045016__9__STARTED_AT_BLOCK=22273407
```

```bash
# chain 1 (Ethereum, Foreign), version 10 -- ABI unchanged; erc20token() flips DAI -> USDS
INTERCHAIN_INDEXER_BRIDGES__3__CONTRACTS__1__0x4aa42145Aa6Ebf72e164C9bBC74fbD3788045016__10__STARTED_AT_BLOCK=23748179
```

```bash
# chain 100 (Gnosis, Home), version 6 -- epoch floor
INTERCHAIN_INDEXER_BRIDGES__3__CONTRACTS__100__0x7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6__6__STARTED_AT_BLOCK=39569937
```

```bash
# chain 100 (Gnosis, Home), version 7 -- UserRequestForSignature gains `token`
INTERCHAIN_INDEXER_BRIDGES__3__CONTRACTS__100__0x7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6__7__STARTED_AT_BLOCK=43027713
```

<details>
<summary>ABIs — 4 variables, inline JSON (trimmed to the subscribed events)</summary>

```bash
# chain 1, version 9 and version 10 (byte-identical ABI)
INTERCHAIN_INDEXER_BRIDGES__3__CONTRACTS__1__0x4aa42145Aa6Ebf72e164C9bBC74fbD3788045016__9__ABI='[{"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"UserRequestForAffirmation","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"transactionHash","type":"bytes32"}],"name":"RelayedMessage","type":"event"}]'
INTERCHAIN_INDEXER_BRIDGES__3__CONTRACTS__1__0x4aa42145Aa6Ebf72e164C9bBC74fbD3788045016__10__ABI='[{"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"UserRequestForAffirmation","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"transactionHash","type":"bytes32"}],"name":"RelayedMessage","type":"event"}]'
```

```bash
# chain 100, version 6 (no `token` on UserRequestForSignature)
INTERCHAIN_INDEXER_BRIDGES__3__CONTRACTS__100__0x7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6__6__ABI='[{"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"UserRequestForSignature","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"AffirmationCompleted","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"SignedForAffirmation","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"}],"name":"SignedForUserRequest","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"name":"authorityResponsibleForRelay","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"},{"indexed":false,"name":"NumberOfCollectedSignatures","type":"uint256"}],"name":"CollectedSignatures","type":"event"}]'
```

```bash
# chain 100, version 7 (UserRequestForSignature gains `token`)
INTERCHAIN_INDEXER_BRIDGES__3__CONTRACTS__100__0x7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6__7__ABI='[{"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"},{"indexed":false,"name":"token","type":"address"}],"name":"UserRequestForSignature","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"AffirmationCompleted","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"SignedForAffirmation","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"}],"name":"SignedForUserRequest","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"name":"authorityResponsibleForRelay","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"},{"indexed":false,"name":"NumberOfCollectedSignatures","type":"uint256"}],"name":"CollectedSignatures","type":"event"}]'
```

</details>

## Testnet set

| Chain | Name | RPC providers |
| --- | --- | --- |
| `11155111` | Sepolia | `tenderly`, `ethpandaops` |
| `10200` | Chiado | `gateway_archive`, `gnosis_official`, `ankr` |

| Bridge | Name | `type` / `indexer_type` | Contracts |
| --- | --- | --- | --- |
| `1003` | xDai Bridge (testnet) | `xdai` / `xdai` | 2 |

### Config files

```bash
INTERCHAIN_INDEXER__CHAINS_CONFIG=config/xdai/chains-testnet.json
INTERCHAIN_INDEXER__BRIDGES_CONFIG=config/xdai/bridges-testnet.json
```

### Chains

#### Chain `11155111` — Sepolia

One variable:

```bash
INTERCHAIN_INDEXER_CHAINS__11155111='{"name":"Sepolia","icon":"https://blockscout-icons.s3.us-east-1.amazonaws.com/ethereum.svg","explorer":{"url":"https://eth-sepolia.blockscout.com"},"rpcs":[{"tenderly":{"url":"https://sepolia.gateway.tenderly.co"},"ethpandaops":{"url":"https://rpc.sepolia.ethpandaops.io"}}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_CHAINS__11155111__NAME=Sepolia
INTERCHAIN_INDEXER_CHAINS__11155111__ICON=https://blockscout-icons.s3.us-east-1.amazonaws.com/ethereum.svg
INTERCHAIN_INDEXER_CHAINS__11155111__EXPLORER__URL=https://eth-sepolia.blockscout.com
# rpc provider "tenderly"
INTERCHAIN_INDEXER_CHAINS__11155111__RPCS__TENDERLY__URL=https://sepolia.gateway.tenderly.co
# rpc provider "ethpandaops"
INTERCHAIN_INDEXER_CHAINS__11155111__RPCS__ETHPANDAOPS__URL=https://rpc.sepolia.ethpandaops.io
```

#### Chain `10200` — Chiado

One variable:

```bash
INTERCHAIN_INDEXER_CHAINS__10200='{"name":"Chiado","icon":"https://blockscout-icons.s3.us-east-1.amazonaws.com/gnosis.svg","explorer":{"url":"https://gnosis-chiado.blockscout.com/"},"rpcs":[{"gateway_archive":{"url":"https://rpc.chiado.gnosis.gateway.fm"},"gnosis_official":{"url":"https://rpc.chiadochain.net"},"ankr":{"url":"https://rpc.ankr.com/gnosis_testnet"}}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_CHAINS__10200__NAME=Chiado
INTERCHAIN_INDEXER_CHAINS__10200__ICON=https://blockscout-icons.s3.us-east-1.amazonaws.com/gnosis.svg
INTERCHAIN_INDEXER_CHAINS__10200__EXPLORER__URL=https://gnosis-chiado.blockscout.com/
# rpc provider "gateway_archive"
INTERCHAIN_INDEXER_CHAINS__10200__RPCS__GATEWAY_ARCHIVE__URL=https://rpc.chiado.gnosis.gateway.fm
# rpc provider "gnosis_official"
INTERCHAIN_INDEXER_CHAINS__10200__RPCS__GNOSIS_OFFICIAL__URL=https://rpc.chiadochain.net
# rpc provider "ankr"
INTERCHAIN_INDEXER_CHAINS__10200__RPCS__ANKR__URL=https://rpc.ankr.com/gnosis_testnet
```

### Bridges

#### Bridge `1003` — xDai Bridge (testnet)

The classic xDai bridge (`ForeignBridgeErcToNative` / `HomeBridgeErcToNative`) between Sepolia and Chiado — ERC-20 locked on Sepolia, native xDAI minted on Chiado. One version window per side. Byte-identical to the `1003` entry in `config/full-testnet/bridges.json`, which a test asserts.

`version` is the proxy's own `EternalStorageProxy.version()` counter, read from the chain: Sepolia reports `2`, Chiado reports `3`. Those are **not** the mainnet bridge's `9`/`10` and `6`/`7` — the counter restarts per deployment, so `indexer/xdai/version.rs` keys its grammar on `(chain_id, side, version)`. A mainnet version number here, or these numbers under a mainnet chain id, is a hard startup error.

`started_at_block` is an **epoch floor**, not just a scan start:

- Sepolia `8239484` — the Foreign v1→v2 upgrade, where `UserRequestForAffirmation` gained its `bytes32` nonce. Below it the event is the two-argument form with no identity field at all.
- Chiado `20553827` — **not** where the Home event shape last changed. The Chiado oracle alternated between nonce-keyed and transaction-hash-keyed `bytes32` inside one implementation window and re-affirmed deposits it had already affirmed the other way. This is the lowest block that excludes every hash-keyed affirmation.

Lowering the Chiado floor is **not** a safe way to index more history: four of the six hash-keyed affirmations below it resolve to Sepolia transactions emitting the modern three-argument source event, which `decode_legacy_source_event` rejects outright, so those blocks would fail and retry forever. The two floors are answers to different questions and are not expected to coincide; `.memory-bank/research/xdai-bridge-testnet-deployment-fit.md` has the full evidence and the per-affirmation breakdown.

Consequence to expect: this bridge indexes four Sepolia deposits and completes one (nonce 3). Nonces 0, 1 and 2 stay `Initiated` because their affirmations are below the Chiado floor. That is the deliberate trade, not a fault.

One variable — both contracts included, since `contracts` is replaced wholesale:

<details>
<summary><code>INTERCHAIN_INDEXER_BRIDGES__1003</code> — 2386 chars (ABIs included)</summary>

```bash
INTERCHAIN_INDEXER_BRIDGES__1003='{"name":"xDai Bridge (testnet)","type":"xdai","indexer_type":"xdai","enabled":true,"api_url":null,"ui_url":null,"docs_url":"https://docs.gnosischain.com/bridges/About%20Token%20Bridges/xdai-bridge","contracts":[{"chain_id":11155111,"address":"0x180Ff98e734415Ecd35faC3d32940e1B45FaD0A2","version":2,"started_at_block":8239484,"kind":null,"abi":"[{\"anonymous\":false,\"inputs\":[{\"indexed\":false,\"name\":\"recipient\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"value\",\"type\":\"uint256\"},{\"indexed\":false,\"name\":\"nonce\",\"type\":\"bytes32\"}],\"name\":\"UserRequestForAffirmation\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":false,\"name\":\"recipient\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"value\",\"type\":\"uint256\"},{\"indexed\":false,\"name\":\"transactionHash\",\"type\":\"bytes32\"}],\"name\":\"RelayedMessage\",\"type\":\"event\"}]"},{"chain_id":10200,"address":"0xccA0Dc2A058884e62082312F09541cC7566406f0","version":3,"started_at_block":20553827,"kind":null,"abi":"[{\"anonymous\":false,\"inputs\":[{\"indexed\":false,\"name\":\"recipient\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"value\",\"type\":\"uint256\"},{\"indexed\":false,\"name\":\"nonce\",\"type\":\"bytes32\"},{\"indexed\":false,\"name\":\"token\",\"type\":\"address\"}],\"name\":\"UserRequestForSignature\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":false,\"name\":\"recipient\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"value\",\"type\":\"uint256\"},{\"indexed\":false,\"name\":\"nonce\",\"type\":\"bytes32\"}],\"name\":\"AffirmationCompleted\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"signer\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"nonce\",\"type\":\"bytes32\"}],\"name\":\"SignedForAffirmation\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"signer\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"messageHash\",\"type\":\"bytes32\"}],\"name\":\"SignedForUserRequest\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":false,\"name\":\"authorityResponsibleForRelay\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"messageHash\",\"type\":\"bytes32\"},{\"indexed\":false,\"name\":\"NumberOfCollectedSignatures\",\"type\":\"uint256\"}],\"name\":\"CollectedSignatures\",\"type\":\"event\"}]"}]}'
```

</details>

Field by field:

```bash
INTERCHAIN_INDEXER_BRIDGES__1003__NAME='xDai Bridge (testnet)'
INTERCHAIN_INDEXER_BRIDGES__1003__TYPE=xdai
INTERCHAIN_INDEXER_BRIDGES__1003__INDEXER_TYPE=xdai
INTERCHAIN_INDEXER_BRIDGES__1003__ENABLED=true
INTERCHAIN_INDEXER_BRIDGES__1003__API_URL=null
INTERCHAIN_INDEXER_BRIDGES__1003__UI_URL=null
INTERCHAIN_INDEXER_BRIDGES__1003__DOCS_URL='https://docs.gnosischain.com/bridges/About%20Token%20Bridges/xdai-bridge'
```

##### Contracts of bridge `1003`

```bash
# chain 11155111 (Sepolia, Foreign), version 2 -- epoch floor: the v1->v2 upgrade
INTERCHAIN_INDEXER_BRIDGES__1003__CONTRACTS__11155111__0x180Ff98e734415Ecd35faC3d32940e1B45FaD0A2__2__STARTED_AT_BLOCK=8239484
```

```bash
# chain 10200 (Chiado, Home), version 3 -- epoch floor: excludes every hash-keyed affirmation
INTERCHAIN_INDEXER_BRIDGES__1003__CONTRACTS__10200__0xccA0Dc2A058884e62082312F09541cC7566406f0__3__STARTED_AT_BLOCK=20553827
```

<details>
<summary>ABIs — 2 variables, inline JSON</summary>

Both are the mainnet xDai ABIs verbatim (the Sepolia entry reuses the mainnet Foreign ABI, the Chiado entry the mainnet Home v7 ABI): every `topic0` was verified identical on the testnet proxies, and `assert_canonical_topics` rejects anything else.

```bash
# chain 11155111, version 2
INTERCHAIN_INDEXER_BRIDGES__1003__CONTRACTS__11155111__0x180Ff98e734415Ecd35faC3d32940e1B45FaD0A2__2__ABI='[{"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"UserRequestForAffirmation","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"transactionHash","type":"bytes32"}],"name":"RelayedMessage","type":"event"}]'
```

```bash
# chain 10200, version 3
INTERCHAIN_INDEXER_BRIDGES__1003__CONTRACTS__10200__0xccA0Dc2A058884e62082312F09541cC7566406f0__3__ABI='[{"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"},{"indexed":false,"name":"token","type":"address"}],"name":"UserRequestForSignature","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"AffirmationCompleted","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"SignedForAffirmation","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"}],"name":"SignedForUserRequest","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"name":"authorityResponsibleForRelay","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"},{"indexed":false,"name":"NumberOfCollectedSignatures","type":"uint256"}],"name":"CollectedSignatures","type":"event"}]'
```

</details>
## Indexer settings

Applies to whichever set is loaded.

```bash
INTERCHAIN_INDEXER__XDAI_INDEXER__PULL_INTERVAL_MS=500
INTERCHAIN_INDEXER__XDAI_INDEXER__BATCH_SIZE=1000
INTERCHAIN_INDEXER__XDAI_INDEXER__RECEIPT_CONCURRENCY=25
```

See `README.md`'s `INTERCHAIN_INDEXER__XDAI_INDEXER__*` block for the full
field reference, including `failure_retry`.
