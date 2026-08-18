# ENVs — `config/full-mainnet`

AMB/Omnibridge between Ethereum and Gnosis, plus Avalanche ICM/ICTT on C-Chain. The set the `just run` recipe uses.

Grammar, merge semantics, field reference and traps: [`config/ENVs.md`](../ENVs.md). Here — only this set's variables, with their actual values, in two interchangeable forms per entity: one JSON variable, or one variable per field.

Every block below stands alone. Copy a chain block to add that chain, a bridge block to add that bridge, a contract block to add one contract version, or a single line for a pinpoint change.

| Chain | Name | RPC providers |
| --- | --- | --- |
| `1` | Ethereum | `gateway`, `drpc`, `1rpc` |
| `100` | Gnosis | `gateway`, `gnosis_official` |
| `43114` | Avalanche C-Chain | `avalanche` |

| Bridge | Name | `type` / `indexer_type` | Contracts |
| --- | --- | --- | --- |
| `1` | AMB/Omnibridge | `amb` / `amb` | 4 |
| `2` | Avalanche ICTT | `avalanche_native` / `icm_ictt` | 1 |

The Avalanche side ships with C-Chain only, deliberately: bridge `2` has
`process_unknown_chains: true`, so subnets are indexed through their C-Chain
counterpart even when unconfigured, and each subnet is then added per deployment
through the environment — see [Optional subnets](#optional-subnets-added-per-deployment).
An instance can therefore drop a subnet by removing its variables, without a config
file changing under it. The subnets that were previously here live in
[`config/avalanche`](../avalanche/ENVs.md), which is the set that indexes them from a
file.

## Config files

```bash
INTERCHAIN_INDEXER__CHAINS_CONFIG=config/full-mainnet/chains.json
INTERCHAIN_INDEXER__BRIDGES_CONFIG=config/full-mainnet/bridges.json
```

## Chains

### Chain `1` — Ethereum

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

### Chain `100` — Gnosis

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

## RPC provider secrets

No provider in `chains.json` declares an `api_key`, and no chain in this set needs one,
so **the set starts with no secret at all**. The one credential that comes up here
belongs to an optional subnet — see
[Optional subnets](#optional-subnets-added-per-deployment).

## Bridges

### Bridge `1` — AMB/Omnibridge

One variable — all 4 contracts included, since `contracts` is replaced wholesale:

<details>
<summary><code>INTERCHAIN_INDEXER_BRIDGES__1</code> — 11323 chars (ABIs included)</summary>

```bash
INTERCHAIN_INDEXER_BRIDGES__1='{"name":"AMB/Omnibridge","type":"amb","indexer_type":"amb","enabled":true,"api_url":null,"ui_url":"https://bridge.gnosischain.com/bridge-explorer/transaction/{{message_id}}","docs_url":"https://docs.gnosischain.com/bridges/About%20Token%20Bridges/amb-bridge","contracts":[{"chain_id":1,"address":"0x4C36d2919e407f0Cc2Ee3c993ccF8ac26d9CE64e","version":6,"started_at_block":20812229,"kind":"amb_proxy","abi":"[{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"messageId\",\"type\":\"bytes32\"},{\"indexed\":false,\"name\":\"encodedData\",\"type\":\"bytes\"}],\"name\":\"UserRequestForAffirmation\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"sender\",\"type\":\"address\"},{\"indexed\":true,\"name\":\"executor\",\"type\":\"address\"},{\"indexed\":true,\"name\":\"messageId\",\"type\":\"bytes32\"},{\"indexed\":false,\"name\":\"status\",\"type\":\"bool\"}],\"name\":\"RelayedMessage\",\"type\":\"event\"}]"},{"chain_id":1,"address":"0x88ad09518695c6c3712AC10a214bE5109a655671","version":6,"started_at_block":13424376,"kind":"omnibridge_mediator","abi":"[{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"bytes32\",\"name\":\"messageId\",\"type\":\"bytes32\"},{\"indexed\":false,\"internalType\":\"address\",\"name\":\"token\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"address\",\"name\":\"recipient\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"value\",\"type\":\"uint256\"}],\"name\":\"FailedMessageFixed\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"nativeToken\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"bridgedToken\",\"type\":\"address\"}],\"name\":\"NewTokenRegistered\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"token\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"recipient\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"value\",\"type\":\"uint256\"},{\"indexed\":true,\"internalType\":\"bytes32\",\"name\":\"messageId\",\"type\":\"bytes32\"}],\"name\":\"TokensBridged\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"token\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"sender\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"value\",\"type\":\"uint256\"},{\"indexed\":true,\"internalType\":\"bytes32\",\"name\":\"messageId\",\"type\":\"bytes32\"}],\"name\":\"TokensBridgingInitiated\",\"type\":\"event\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"string\",\"name\":\"_name\",\"type\":\"string\"},{\"internalType\":\"string\",\"name\":\"_symbol\",\"type\":\"string\"},{\"internalType\":\"uint8\",\"name\":\"_decimals\",\"type\":\"uint8\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"deployAndHandleBridgedTokens\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"string\",\"name\":\"_name\",\"type\":\"string\"},{\"internalType\":\"string\",\"name\":\"_symbol\",\"type\":\"string\"},{\"internalType\":\"uint8\",\"name\":\"_decimals\",\"type\":\"uint8\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"},{\"internalType\":\"bytes\",\"name\":\"_data\",\"type\":\"bytes\"}],\"name\":\"deployAndHandleBridgedTokensAndCall\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"handleBridgedTokens\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"},{\"internalType\":\"bytes\",\"name\":\"_data\",\"type\":\"bytes\"}],\"name\":\"handleBridgedTokensAndCall\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"handleNativeTokens\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"},{\"internalType\":\"bytes\",\"name\":\"_data\",\"type\":\"bytes\"}],\"name\":\"handleNativeTokensAndCall\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]"},{"chain_id":100,"address":"0x75Df5AF045d91108662D8080fD1FEFAd6aA0bb59","version":6,"started_at_block":36145833,"kind":"amb_proxy","abi":"[{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"messageId\",\"type\":\"bytes32\"},{\"indexed\":false,\"name\":\"encodedData\",\"type\":\"bytes\"}],\"name\":\"UserRequestForSignature\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"sender\",\"type\":\"address\"},{\"indexed\":true,\"name\":\"executor\",\"type\":\"address\"},{\"indexed\":true,\"name\":\"messageId\",\"type\":\"bytes32\"},{\"indexed\":false,\"name\":\"status\",\"type\":\"bool\"}],\"name\":\"AffirmationCompleted\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"signer\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"messageHash\",\"type\":\"bytes32\"}],\"name\":\"SignedForUserRequest\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"signer\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"messageHash\",\"type\":\"bytes32\"}],\"name\":\"SignedForAffirmation\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":false,\"name\":\"authorityResponsibleForRelay\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"messageHash\",\"type\":\"bytes32\"},{\"indexed\":false,\"name\":\"NumberOfCollectedSignatures\",\"type\":\"uint256\"}],\"name\":\"CollectedSignatures\",\"type\":\"event\"}]"},{"chain_id":100,"address":"0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d","version":8,"started_at_block":18588922,"kind":"omnibridge_mediator","abi":"[{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"bytes32\",\"name\":\"messageId\",\"type\":\"bytes32\"},{\"indexed\":false,\"internalType\":\"address\",\"name\":\"token\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"address\",\"name\":\"recipient\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"value\",\"type\":\"uint256\"}],\"name\":\"FailedMessageFixed\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"nativeToken\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"bridgedToken\",\"type\":\"address\"}],\"name\":\"NewTokenRegistered\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"token\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"recipient\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"value\",\"type\":\"uint256\"},{\"indexed\":true,\"internalType\":\"bytes32\",\"name\":\"messageId\",\"type\":\"bytes32\"}],\"name\":\"TokensBridged\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"token\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"sender\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"value\",\"type\":\"uint256\"},{\"indexed\":true,\"internalType\":\"bytes32\",\"name\":\"messageId\",\"type\":\"bytes32\"}],\"name\":\"TokensBridgingInitiated\",\"type\":\"event\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"string\",\"name\":\"_name\",\"type\":\"string\"},{\"internalType\":\"string\",\"name\":\"_symbol\",\"type\":\"string\"},{\"internalType\":\"uint8\",\"name\":\"_decimals\",\"type\":\"uint8\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"deployAndHandleBridgedTokens\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"string\",\"name\":\"_name\",\"type\":\"string\"},{\"internalType\":\"string\",\"name\":\"_symbol\",\"type\":\"string\"},{\"internalType\":\"uint8\",\"name\":\"_decimals\",\"type\":\"uint8\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"},{\"internalType\":\"bytes\",\"name\":\"_data\",\"type\":\"bytes\"}],\"name\":\"deployAndHandleBridgedTokensAndCall\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"handleBridgedTokens\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"},{\"internalType\":\"bytes\",\"name\":\"_data\",\"type\":\"bytes\"}],\"name\":\"handleBridgedTokensAndCall\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"handleNativeTokens\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"},{\"internalType\":\"bytes\",\"name\":\"_data\",\"type\":\"bytes\"}],\"name\":\"handleNativeTokensAndCall\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]"}]}'
```

</details>

Field by field:

```bash
INTERCHAIN_INDEXER_BRIDGES__1__NAME=AMB/Omnibridge
INTERCHAIN_INDEXER_BRIDGES__1__TYPE=amb
INTERCHAIN_INDEXER_BRIDGES__1__INDEXER_TYPE=amb
INTERCHAIN_INDEXER_BRIDGES__1__ENABLED=true
INTERCHAIN_INDEXER_BRIDGES__1__API_URL=null
INTERCHAIN_INDEXER_BRIDGES__1__UI_URL='https://bridge.gnosischain.com/bridge-explorer/transaction/{{message_id}}'
INTERCHAIN_INDEXER_BRIDGES__1__DOCS_URL='https://docs.gnosischain.com/bridges/About%20Token%20Bridges/amb-bridge'
```

#### Contracts of bridge `1`

```bash
# chain 1, version 6, amb_proxy
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__1__0x4C36d2919e407f0Cc2Ee3c993ccF8ac26d9CE64e__6__STARTED_AT_BLOCK=20812229
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__1__0x4C36d2919e407f0Cc2Ee3c993ccF8ac26d9CE64e__6__KIND=amb_proxy
```

```bash
# chain 1, version 6, omnibridge_mediator
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__1__0x88ad09518695c6c3712AC10a214bE5109a655671__6__STARTED_AT_BLOCK=13424376
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__1__0x88ad09518695c6c3712AC10a214bE5109a655671__6__KIND=omnibridge_mediator
```

```bash
# chain 100, version 6, amb_proxy
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0x75Df5AF045d91108662D8080fD1FEFAd6aA0bb59__6__STARTED_AT_BLOCK=36145833
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0x75Df5AF045d91108662D8080fD1FEFAd6aA0bb59__6__KIND=amb_proxy
```

```bash
# chain 100, version 8, omnibridge_mediator
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d__8__STARTED_AT_BLOCK=18588922
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d__8__KIND=omnibridge_mediator
```

<details>
<summary>ABIs — 4 variables, inline JSON</summary>

```bash
# chain 1, version 6, amb_proxy
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__1__0x4C36d2919e407f0Cc2Ee3c993ccF8ac26d9CE64e__6__ABI='[{"anonymous":false,"inputs":[{"indexed":true,"name":"messageId","type":"bytes32"},{"indexed":false,"name":"encodedData","type":"bytes"}],"name":"UserRequestForAffirmation","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"sender","type":"address"},{"indexed":true,"name":"executor","type":"address"},{"indexed":true,"name":"messageId","type":"bytes32"},{"indexed":false,"name":"status","type":"bool"}],"name":"RelayedMessage","type":"event"}]'
```

```bash
# chain 1, version 6, omnibridge_mediator
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__1__0x88ad09518695c6c3712AC10a214bE5109a655671__6__ABI='[{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes32","name":"messageId","type":"bytes32"},{"indexed":false,"internalType":"address","name":"token","type":"address"},{"indexed":false,"internalType":"address","name":"recipient","type":"address"},{"indexed":false,"internalType":"uint256","name":"value","type":"uint256"}],"name":"FailedMessageFixed","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"nativeToken","type":"address"},{"indexed":true,"internalType":"address","name":"bridgedToken","type":"address"}],"name":"NewTokenRegistered","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"token","type":"address"},{"indexed":true,"internalType":"address","name":"recipient","type":"address"},{"indexed":false,"internalType":"uint256","name":"value","type":"uint256"},{"indexed":true,"internalType":"bytes32","name":"messageId","type":"bytes32"}],"name":"TokensBridged","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"token","type":"address"},{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":false,"internalType":"uint256","name":"value","type":"uint256"},{"indexed":true,"internalType":"bytes32","name":"messageId","type":"bytes32"}],"name":"TokensBridgingInitiated","type":"event"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"string","name":"_name","type":"string"},{"internalType":"string","name":"_symbol","type":"string"},{"internalType":"uint8","name":"_decimals","type":"uint8"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"}],"name":"deployAndHandleBridgedTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"string","name":"_name","type":"string"},{"internalType":"string","name":"_symbol","type":"string"},{"internalType":"uint8","name":"_decimals","type":"uint8"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"},{"internalType":"bytes","name":"_data","type":"bytes"}],"name":"deployAndHandleBridgedTokensAndCall","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"}],"name":"handleBridgedTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"},{"internalType":"bytes","name":"_data","type":"bytes"}],"name":"handleBridgedTokensAndCall","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"}],"name":"handleNativeTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"},{"internalType":"bytes","name":"_data","type":"bytes"}],"name":"handleNativeTokensAndCall","outputs":[],"stateMutability":"nonpayable","type":"function"}]'
```

```bash
# chain 100, version 6, amb_proxy
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0x75Df5AF045d91108662D8080fD1FEFAd6aA0bb59__6__ABI='[{"anonymous":false,"inputs":[{"indexed":true,"name":"messageId","type":"bytes32"},{"indexed":false,"name":"encodedData","type":"bytes"}],"name":"UserRequestForSignature","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"sender","type":"address"},{"indexed":true,"name":"executor","type":"address"},{"indexed":true,"name":"messageId","type":"bytes32"},{"indexed":false,"name":"status","type":"bool"}],"name":"AffirmationCompleted","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"}],"name":"SignedForUserRequest","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"}],"name":"SignedForAffirmation","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"name":"authorityResponsibleForRelay","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"},{"indexed":false,"name":"NumberOfCollectedSignatures","type":"uint256"}],"name":"CollectedSignatures","type":"event"}]'
```

```bash
# chain 100, version 8, omnibridge_mediator
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__100__0xf6A78083ca3e2a662D6dd1703c939c8aCE2e268d__8__ABI='[{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes32","name":"messageId","type":"bytes32"},{"indexed":false,"internalType":"address","name":"token","type":"address"},{"indexed":false,"internalType":"address","name":"recipient","type":"address"},{"indexed":false,"internalType":"uint256","name":"value","type":"uint256"}],"name":"FailedMessageFixed","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"nativeToken","type":"address"},{"indexed":true,"internalType":"address","name":"bridgedToken","type":"address"}],"name":"NewTokenRegistered","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"token","type":"address"},{"indexed":true,"internalType":"address","name":"recipient","type":"address"},{"indexed":false,"internalType":"uint256","name":"value","type":"uint256"},{"indexed":true,"internalType":"bytes32","name":"messageId","type":"bytes32"}],"name":"TokensBridged","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"token","type":"address"},{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":false,"internalType":"uint256","name":"value","type":"uint256"},{"indexed":true,"internalType":"bytes32","name":"messageId","type":"bytes32"}],"name":"TokensBridgingInitiated","type":"event"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"string","name":"_name","type":"string"},{"internalType":"string","name":"_symbol","type":"string"},{"internalType":"uint8","name":"_decimals","type":"uint8"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"}],"name":"deployAndHandleBridgedTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"string","name":"_name","type":"string"},{"internalType":"string","name":"_symbol","type":"string"},{"internalType":"uint8","name":"_decimals","type":"uint8"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"},{"internalType":"bytes","name":"_data","type":"bytes"}],"name":"deployAndHandleBridgedTokensAndCall","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"}],"name":"handleBridgedTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"},{"internalType":"bytes","name":"_data","type":"bytes"}],"name":"handleBridgedTokensAndCall","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"}],"name":"handleNativeTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"},{"internalType":"bytes","name":"_data","type":"bytes"}],"name":"handleNativeTokensAndCall","outputs":[],"stateMutability":"nonpayable","type":"function"}]'
```

</details>

### Bridge `2` — Avalanche ICTT

One variable — the single contract included, since `contracts` is replaced wholesale:

```bash
INTERCHAIN_INDEXER_BRIDGES__2='{"name":"Avalanche ICTT","type":"avalanche_native","indexer_type":"icm_ictt","enabled":true,"api_url":null,"ui_url":null,"docs_url":null,"process_unknown_chains":true,"home_chain_id":null,"contracts":[{"chain_id":43114,"address":"0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf","version":1,"started_at_block":42526120}]}'
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
INTERCHAIN_INDEXER_BRIDGES__2__PROCESS_UNKNOWN_CHAINS=true
INTERCHAIN_INDEXER_BRIDGES__2__HOME_CHAIN_ID=null
```

#### Contracts of bridge `2`

```bash
# chain 43114, version 1
INTERCHAIN_INDEXER_BRIDGES__2__CONTRACTS__43114__0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf__1__STARTED_AT_BLOCK=42526120
```

## Optional subnets, added per deployment

Neither subnet below is in `chains.json` or `bridges.json`. Each deployment adds the
ones it wants through the environment, and drops one by removing its variables — no
config file changes under a running instance, which is the point of keeping them out.

Adding a subnet takes **two** blocks: the chain, so the indexer has an RPC and explorer
for it, and a bridge `2` contract entry, so its own ICTT contract is scanned. Contract
entries merge per-entry (`…__CONTRACTS__<CHAIN>__<ADDRESS>__<VERSION>__…`), so adding
one leaves the C-Chain entry alone — unlike `…__CONTRACTS='[…]'`, which replaces the
array wholesale.

Leaving a subnet out does not hide its messages: bridge `2` sets
`process_unknown_chains: true`, so ICTT traffic to and from an unconfigured chain is
still indexed from the C-Chain side. What the subnet's own entry adds is scanning that
chain directly.

Both subnets are indexed from a file — no environment needed — by
[`config/avalanche`](../avalanche/ENVs.md).

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
# rpc provider "glacier"
INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__URL=https://glacier-api.avax.network/v1/ext/bc/8021/rpc
INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__MAX_RPS=1
INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__MULTICALL_BATCHING_US=0
```

Its bridge `2` contract:

```bash
INTERCHAIN_INDEXER_BRIDGES__2__CONTRACTS__8021__0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf__1__STARTED_AT_BLOCK=4
```

Glacier accepts an `x-glacier-api-key` header and rate-limits harder without one. The
credential is optional; supply the shape *and* the value together, since a declared
`api_key` whose value is unset, empty or whitespace-only fails startup by design:

```bash
INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__API_KEY__LOCATION=header
INTERCHAIN_INDEXER_CHAINS__8021__RPCS__GLACIER__API_KEY__PARAM_NAME=x-glacier-api-key
INTERCHAIN_INDEXER_RPC_API_KEY__8021__GLACIER=<secret>
```

Raising `MAX_RPS` above `1` usually goes with the key. Locally all of this goes in
`.env` (see [`.env.example`](../../.env.example)) and the service is started with
`just run-dev`. Nothing loads `.env` implicitly — deliberately, since a globally loaded
`.env` would also reach `just test`, whose fixture declares chains `1` and `100` only
and fails on a partial chain `8021` appended to it.

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

Its bridge `2` contract:

```bash
INTERCHAIN_INDEXER_BRIDGES__2__CONTRACTS__68414__0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf__1__STARTED_AT_BLOCK=4
```
