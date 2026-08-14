# ENVs — `config/omnibridge`

AMB/Omnibridge only, in two sets: **mainnet** (Ethereum ↔ Gnosis) and **testnet** (Sepolia ↔ Chiado). The mainnet pair is the AMB-only subset of `config/full-mainnet`: identical bridge and contracts, the same two chains minus the Blockscout RPC nodes. The testnet pair is identical to `config/full-testnet`.

Grammar, merge semantics, field reference and traps: [`config/ENVs.md`](../ENVs.md). Here — only these sets' variables, with their actual values, in two interchangeable forms per entity: one JSON variable, or one variable per field.

Every block below stands alone. Copy a chain block to add that chain, a bridge block to add that bridge, a contract block to add one contract version, or a single line for a pinpoint change.

## Mainnet set

| Chain | Name | RPC providers |
| --- | --- | --- |
| `1` | Ethereum | `gateway`, `drpc`, `1rpc` |
| `100` | Gnosis | `gateway`, `gnosis_official` |

| Bridge | Name | `type` / `indexer_type` | Contracts |
| --- | --- | --- | --- |
| `1` | AMB/Omnibridge | `amb` / `amb` | 4 |

### Config files

```bash
INTERCHAIN_INDEXER__CHAINS_CONFIG=config/omnibridge/chains.json
INTERCHAIN_INDEXER__BRIDGES_CONFIG=config/omnibridge/bridges.json
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

#### Bridge `1` — AMB/Omnibridge

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

##### Contracts of bridge `1`

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

## Testnet set

| Chain | Name | RPC providers |
| --- | --- | --- |
| `11155111` | Sepolia | `tenderly`, `drpc` |
| `10200` | Chiado | `gateway_archive`, `gnosis_official`, `ankr` |

| Bridge | Name | `type` / `indexer_type` | Contracts |
| --- | --- | --- | --- |
| `1` | AMB/Omnibridge | `amb` / `amb` | 4 |

### Config files

```bash
INTERCHAIN_INDEXER__CHAINS_CONFIG=config/omnibridge/chains-testnet.json
INTERCHAIN_INDEXER__BRIDGES_CONFIG=config/omnibridge/bridges-testnet.json
```

### Chains

#### Chain `11155111` — Sepolia

One variable:

```bash
INTERCHAIN_INDEXER_CHAINS__11155111='{"name":"Sepolia","icon":"https://blockscout-icons.s3.us-east-1.amazonaws.com/ethereum.svg","explorer":{"url":"https://eth-sepolia.blockscout.com"},"rpcs":[{"tenderly":{"url":"https://sepolia.gateway.tenderly.co"},"drpc":{"url":"https://sepolia.drpc.org"}}]}'
```

Field by field:

```bash
INTERCHAIN_INDEXER_CHAINS__11155111__NAME=Sepolia
INTERCHAIN_INDEXER_CHAINS__11155111__ICON=https://blockscout-icons.s3.us-east-1.amazonaws.com/ethereum.svg
INTERCHAIN_INDEXER_CHAINS__11155111__EXPLORER__URL=https://eth-sepolia.blockscout.com
# rpc provider "tenderly"
INTERCHAIN_INDEXER_CHAINS__11155111__RPCS__TENDERLY__URL=https://sepolia.gateway.tenderly.co
# rpc provider "drpc"
INTERCHAIN_INDEXER_CHAINS__11155111__RPCS__DRPC__URL=https://sepolia.drpc.org
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

#### Bridge `1` — AMB/Omnibridge

One variable — all 4 contracts included, since `contracts` is replaced wholesale:

<details>
<summary><code>INTERCHAIN_INDEXER_BRIDGES__1</code> — 11266 chars (ABIs included)</summary>

```bash
INTERCHAIN_INDEXER_BRIDGES__1='{"name":"AMB/Omnibridge","type":"amb","indexer_type":"amb","enabled":true,"api_url":null,"ui_url":null,"docs_url":"https://docs.gnosischain.com/bridges/About%20Token%20Bridges/amb-bridge","contracts":[{"chain_id":11155111,"address":"0xf2546d6648bd2af6a008a7e7c1542bb240329e11","version":6,"started_at_block":5272294,"kind":"amb_proxy","abi":"[{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"messageId\",\"type\":\"bytes32\"},{\"indexed\":false,\"name\":\"encodedData\",\"type\":\"bytes\"}],\"name\":\"UserRequestForAffirmation\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"sender\",\"type\":\"address\"},{\"indexed\":true,\"name\":\"executor\",\"type\":\"address\"},{\"indexed\":true,\"name\":\"messageId\",\"type\":\"bytes32\"},{\"indexed\":false,\"name\":\"status\",\"type\":\"bool\"}],\"name\":\"RelayedMessage\",\"type\":\"event\"}]"},{"chain_id":11155111,"address":"0x63e47c5e3303dddcaf3b404b1ccf9eb633652e9e","version":6,"started_at_block":5272539,"kind":"omnibridge_mediator","abi":"[{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"bytes32\",\"name\":\"messageId\",\"type\":\"bytes32\"},{\"indexed\":false,\"internalType\":\"address\",\"name\":\"token\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"address\",\"name\":\"recipient\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"value\",\"type\":\"uint256\"}],\"name\":\"FailedMessageFixed\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"nativeToken\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"bridgedToken\",\"type\":\"address\"}],\"name\":\"NewTokenRegistered\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"token\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"recipient\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"value\",\"type\":\"uint256\"},{\"indexed\":true,\"internalType\":\"bytes32\",\"name\":\"messageId\",\"type\":\"bytes32\"}],\"name\":\"TokensBridged\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"token\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"sender\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"value\",\"type\":\"uint256\"},{\"indexed\":true,\"internalType\":\"bytes32\",\"name\":\"messageId\",\"type\":\"bytes32\"}],\"name\":\"TokensBridgingInitiated\",\"type\":\"event\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"string\",\"name\":\"_name\",\"type\":\"string\"},{\"internalType\":\"string\",\"name\":\"_symbol\",\"type\":\"string\"},{\"internalType\":\"uint8\",\"name\":\"_decimals\",\"type\":\"uint8\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"deployAndHandleBridgedTokens\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"string\",\"name\":\"_name\",\"type\":\"string\"},{\"internalType\":\"string\",\"name\":\"_symbol\",\"type\":\"string\"},{\"internalType\":\"uint8\",\"name\":\"_decimals\",\"type\":\"uint8\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"},{\"internalType\":\"bytes\",\"name\":\"_data\",\"type\":\"bytes\"}],\"name\":\"deployAndHandleBridgedTokensAndCall\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"handleBridgedTokens\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"},{\"internalType\":\"bytes\",\"name\":\"_data\",\"type\":\"bytes\"}],\"name\":\"handleBridgedTokensAndCall\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"handleNativeTokens\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"},{\"internalType\":\"bytes\",\"name\":\"_data\",\"type\":\"bytes\"}],\"name\":\"handleNativeTokensAndCall\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]"},{"chain_id":10200,"address":"0x8448E15d0e706C0298dECA99F0b4744030e59d7d","version":6,"started_at_block":8199150,"kind":"amb_proxy","abi":"[{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"messageId\",\"type\":\"bytes32\"},{\"indexed\":false,\"name\":\"encodedData\",\"type\":\"bytes\"}],\"name\":\"UserRequestForSignature\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"sender\",\"type\":\"address\"},{\"indexed\":true,\"name\":\"executor\",\"type\":\"address\"},{\"indexed\":true,\"name\":\"messageId\",\"type\":\"bytes32\"},{\"indexed\":false,\"name\":\"status\",\"type\":\"bool\"}],\"name\":\"AffirmationCompleted\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"signer\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"messageHash\",\"type\":\"bytes32\"}],\"name\":\"SignedForUserRequest\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"name\":\"signer\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"messageHash\",\"type\":\"bytes32\"}],\"name\":\"SignedForAffirmation\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":false,\"name\":\"authorityResponsibleForRelay\",\"type\":\"address\"},{\"indexed\":false,\"name\":\"messageHash\",\"type\":\"bytes32\"},{\"indexed\":false,\"name\":\"NumberOfCollectedSignatures\",\"type\":\"uint256\"}],\"name\":\"CollectedSignatures\",\"type\":\"event\"}]"},{"chain_id":10200,"address":"0x82f63B9730f419CbfEEF10d58a522203838d74c8","version":8,"started_at_block":8199827,"kind":"omnibridge_mediator","abi":"[{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"bytes32\",\"name\":\"messageId\",\"type\":\"bytes32\"},{\"indexed\":false,\"internalType\":\"address\",\"name\":\"token\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"address\",\"name\":\"recipient\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"value\",\"type\":\"uint256\"}],\"name\":\"FailedMessageFixed\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"nativeToken\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"bridgedToken\",\"type\":\"address\"}],\"name\":\"NewTokenRegistered\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"token\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"recipient\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"value\",\"type\":\"uint256\"},{\"indexed\":true,\"internalType\":\"bytes32\",\"name\":\"messageId\",\"type\":\"bytes32\"}],\"name\":\"TokensBridged\",\"type\":\"event\"},{\"anonymous\":false,\"inputs\":[{\"indexed\":true,\"internalType\":\"address\",\"name\":\"token\",\"type\":\"address\"},{\"indexed\":true,\"internalType\":\"address\",\"name\":\"sender\",\"type\":\"address\"},{\"indexed\":false,\"internalType\":\"uint256\",\"name\":\"value\",\"type\":\"uint256\"},{\"indexed\":true,\"internalType\":\"bytes32\",\"name\":\"messageId\",\"type\":\"bytes32\"}],\"name\":\"TokensBridgingInitiated\",\"type\":\"event\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"string\",\"name\":\"_name\",\"type\":\"string\"},{\"internalType\":\"string\",\"name\":\"_symbol\",\"type\":\"string\"},{\"internalType\":\"uint8\",\"name\":\"_decimals\",\"type\":\"uint8\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"deployAndHandleBridgedTokens\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"string\",\"name\":\"_name\",\"type\":\"string\"},{\"internalType\":\"string\",\"name\":\"_symbol\",\"type\":\"string\"},{\"internalType\":\"uint8\",\"name\":\"_decimals\",\"type\":\"uint8\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"},{\"internalType\":\"bytes\",\"name\":\"_data\",\"type\":\"bytes\"}],\"name\":\"deployAndHandleBridgedTokensAndCall\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"handleBridgedTokens\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"},{\"internalType\":\"bytes\",\"name\":\"_data\",\"type\":\"bytes\"}],\"name\":\"handleBridgedTokensAndCall\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"}],\"name\":\"handleNativeTokens\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"},{\"inputs\":[{\"internalType\":\"address\",\"name\":\"_token\",\"type\":\"address\"},{\"internalType\":\"address\",\"name\":\"_recipient\",\"type\":\"address\"},{\"internalType\":\"uint256\",\"name\":\"_value\",\"type\":\"uint256\"},{\"internalType\":\"bytes\",\"name\":\"_data\",\"type\":\"bytes\"}],\"name\":\"handleNativeTokensAndCall\",\"outputs\":[],\"stateMutability\":\"nonpayable\",\"type\":\"function\"}]"}]}'
```

</details>

Field by field:

```bash
INTERCHAIN_INDEXER_BRIDGES__1__NAME=AMB/Omnibridge
INTERCHAIN_INDEXER_BRIDGES__1__TYPE=amb
INTERCHAIN_INDEXER_BRIDGES__1__INDEXER_TYPE=amb
INTERCHAIN_INDEXER_BRIDGES__1__ENABLED=true
INTERCHAIN_INDEXER_BRIDGES__1__API_URL=null
INTERCHAIN_INDEXER_BRIDGES__1__UI_URL=null
INTERCHAIN_INDEXER_BRIDGES__1__DOCS_URL='https://docs.gnosischain.com/bridges/About%20Token%20Bridges/amb-bridge'
```

##### Contracts of bridge `1`

```bash
# chain 11155111, version 6, amb_proxy
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__11155111__0xf2546d6648bd2af6a008a7e7c1542bb240329e11__6__STARTED_AT_BLOCK=5272294
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__11155111__0xf2546d6648bd2af6a008a7e7c1542bb240329e11__6__KIND=amb_proxy
```

```bash
# chain 11155111, version 6, omnibridge_mediator
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__11155111__0x63e47c5e3303dddcaf3b404b1ccf9eb633652e9e__6__STARTED_AT_BLOCK=5272539
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__11155111__0x63e47c5e3303dddcaf3b404b1ccf9eb633652e9e__6__KIND=omnibridge_mediator
```

```bash
# chain 10200, version 6, amb_proxy
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__10200__0x8448E15d0e706C0298dECA99F0b4744030e59d7d__6__STARTED_AT_BLOCK=8199150
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__10200__0x8448E15d0e706C0298dECA99F0b4744030e59d7d__6__KIND=amb_proxy
```

```bash
# chain 10200, version 8, omnibridge_mediator
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__10200__0x82f63B9730f419CbfEEF10d58a522203838d74c8__8__STARTED_AT_BLOCK=8199827
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__10200__0x82f63B9730f419CbfEEF10d58a522203838d74c8__8__KIND=omnibridge_mediator
```

<details>
<summary>ABIs — 4 variables, inline JSON</summary>

```bash
# chain 11155111, version 6, amb_proxy
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__11155111__0xf2546d6648bd2af6a008a7e7c1542bb240329e11__6__ABI='[{"anonymous":false,"inputs":[{"indexed":true,"name":"messageId","type":"bytes32"},{"indexed":false,"name":"encodedData","type":"bytes"}],"name":"UserRequestForAffirmation","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"sender","type":"address"},{"indexed":true,"name":"executor","type":"address"},{"indexed":true,"name":"messageId","type":"bytes32"},{"indexed":false,"name":"status","type":"bool"}],"name":"RelayedMessage","type":"event"}]'
```

```bash
# chain 11155111, version 6, omnibridge_mediator
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__11155111__0x63e47c5e3303dddcaf3b404b1ccf9eb633652e9e__6__ABI='[{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes32","name":"messageId","type":"bytes32"},{"indexed":false,"internalType":"address","name":"token","type":"address"},{"indexed":false,"internalType":"address","name":"recipient","type":"address"},{"indexed":false,"internalType":"uint256","name":"value","type":"uint256"}],"name":"FailedMessageFixed","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"nativeToken","type":"address"},{"indexed":true,"internalType":"address","name":"bridgedToken","type":"address"}],"name":"NewTokenRegistered","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"token","type":"address"},{"indexed":true,"internalType":"address","name":"recipient","type":"address"},{"indexed":false,"internalType":"uint256","name":"value","type":"uint256"},{"indexed":true,"internalType":"bytes32","name":"messageId","type":"bytes32"}],"name":"TokensBridged","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"token","type":"address"},{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":false,"internalType":"uint256","name":"value","type":"uint256"},{"indexed":true,"internalType":"bytes32","name":"messageId","type":"bytes32"}],"name":"TokensBridgingInitiated","type":"event"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"string","name":"_name","type":"string"},{"internalType":"string","name":"_symbol","type":"string"},{"internalType":"uint8","name":"_decimals","type":"uint8"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"}],"name":"deployAndHandleBridgedTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"string","name":"_name","type":"string"},{"internalType":"string","name":"_symbol","type":"string"},{"internalType":"uint8","name":"_decimals","type":"uint8"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"},{"internalType":"bytes","name":"_data","type":"bytes"}],"name":"deployAndHandleBridgedTokensAndCall","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"}],"name":"handleBridgedTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"},{"internalType":"bytes","name":"_data","type":"bytes"}],"name":"handleBridgedTokensAndCall","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"}],"name":"handleNativeTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"},{"internalType":"bytes","name":"_data","type":"bytes"}],"name":"handleNativeTokensAndCall","outputs":[],"stateMutability":"nonpayable","type":"function"}]'
```

```bash
# chain 10200, version 6, amb_proxy
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__10200__0x8448E15d0e706C0298dECA99F0b4744030e59d7d__6__ABI='[{"anonymous":false,"inputs":[{"indexed":true,"name":"messageId","type":"bytes32"},{"indexed":false,"name":"encodedData","type":"bytes"}],"name":"UserRequestForSignature","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"sender","type":"address"},{"indexed":true,"name":"executor","type":"address"},{"indexed":true,"name":"messageId","type":"bytes32"},{"indexed":false,"name":"status","type":"bool"}],"name":"AffirmationCompleted","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"}],"name":"SignedForUserRequest","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"}],"name":"SignedForAffirmation","type":"event"},{"anonymous":false,"inputs":[{"indexed":false,"name":"authorityResponsibleForRelay","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"},{"indexed":false,"name":"NumberOfCollectedSignatures","type":"uint256"}],"name":"CollectedSignatures","type":"event"}]'
```

```bash
# chain 10200, version 8, omnibridge_mediator
INTERCHAIN_INDEXER_BRIDGES__1__CONTRACTS__10200__0x82f63B9730f419CbfEEF10d58a522203838d74c8__8__ABI='[{"anonymous":false,"inputs":[{"indexed":true,"internalType":"bytes32","name":"messageId","type":"bytes32"},{"indexed":false,"internalType":"address","name":"token","type":"address"},{"indexed":false,"internalType":"address","name":"recipient","type":"address"},{"indexed":false,"internalType":"uint256","name":"value","type":"uint256"}],"name":"FailedMessageFixed","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"nativeToken","type":"address"},{"indexed":true,"internalType":"address","name":"bridgedToken","type":"address"}],"name":"NewTokenRegistered","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"token","type":"address"},{"indexed":true,"internalType":"address","name":"recipient","type":"address"},{"indexed":false,"internalType":"uint256","name":"value","type":"uint256"},{"indexed":true,"internalType":"bytes32","name":"messageId","type":"bytes32"}],"name":"TokensBridged","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"internalType":"address","name":"token","type":"address"},{"indexed":true,"internalType":"address","name":"sender","type":"address"},{"indexed":false,"internalType":"uint256","name":"value","type":"uint256"},{"indexed":true,"internalType":"bytes32","name":"messageId","type":"bytes32"}],"name":"TokensBridgingInitiated","type":"event"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"string","name":"_name","type":"string"},{"internalType":"string","name":"_symbol","type":"string"},{"internalType":"uint8","name":"_decimals","type":"uint8"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"}],"name":"deployAndHandleBridgedTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"string","name":"_name","type":"string"},{"internalType":"string","name":"_symbol","type":"string"},{"internalType":"uint8","name":"_decimals","type":"uint8"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"},{"internalType":"bytes","name":"_data","type":"bytes"}],"name":"deployAndHandleBridgedTokensAndCall","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"}],"name":"handleBridgedTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"},{"internalType":"bytes","name":"_data","type":"bytes"}],"name":"handleBridgedTokensAndCall","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"}],"name":"handleNativeTokens","outputs":[],"stateMutability":"nonpayable","type":"function"},{"inputs":[{"internalType":"address","name":"_token","type":"address"},{"internalType":"address","name":"_recipient","type":"address"},{"internalType":"uint256","name":"_value","type":"uint256"},{"internalType":"bytes","name":"_data","type":"bytes"}],"name":"handleNativeTokensAndCall","outputs":[],"stateMutability":"nonpayable","type":"function"}]'
```

</details>
