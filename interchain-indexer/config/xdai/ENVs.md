# ENVs — `config/xdai`

xDai bridge only (Ethereum ↔ Gnosis, `bridge_id` `3`). Same two chains as
`config/omnibridge`, but only the xDai bridge's four contract versions.

Grammar, merge semantics, field reference and traps: [`config/ENVs.md`](../ENVs.md). Here — only this set's variables, with their actual values, in two interchangeable forms per entity: one JSON variable, or one variable per field.

Every block below stands alone. Copy a chain block to add that chain, the bridge block to add the bridge, a contract block to add one contract version, or a single line for a pinpoint change.

## Chains

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

### Indexer settings

```bash
INTERCHAIN_INDEXER__XDAI_INDEXER__PULL_INTERVAL_MS=500
INTERCHAIN_INDEXER__XDAI_INDEXER__BATCH_SIZE=1000
INTERCHAIN_INDEXER__XDAI_INDEXER__RECEIPT_CONCURRENCY=25
```

See `README.md`'s `INTERCHAIN_INDEXER__XDAI_INDEXER__*` block for the full
field reference, including `failure_retry`.
