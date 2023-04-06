# <h1 align="center"> Smart-contract Verifier (HTTP Server) - DEPRECATED</h1>

**The HTTP server implementation is deprecated and was 
replaced by combined HTTP and GRPC server. Use this version 
only if you require current version of API.**

**See [smart-contract-verifier-server](../smart-contract-verifier-server)**

----------

Smart-contract verification service. Runs as an HTTP server and allows
making verification requests through REST API. It is stateless
and answers requests based on provided information only.

## Configuration

Service supports configuration via configuration file and environment variables.
The latter overwrites the former in case if both are provided. For all missing fields
default values are used (if possible).

### Configuration file

Service uses a configuration file the path to which is specified via `SMART_CONTRACT_VERIFIER__CONFIG=[path]` environment variable.
The base configuration file with all available options could be found at [config/base.toml](./config/base.toml).

Below is an example of a simple configuration file which is filled with default values.

```toml
[server]
# IP address and port number the server should listen to
addr = "0.0.0.0:8043"

[solidity]
# When disabled, solidity related handlers are not available
enabled = true
# A directory where compilers would be downloaded to
compilers_dir = "/tmp/solidity-compilers"
# List of avaialble solidity versions updates cron formatted schedule 
refresh_versions_schedule = "0 0 * * * * *"

[solidity.fetcher.list]
# List of all available solidity compilers and information about them.
list_url = "https://solc-bin.ethereum.org/linux-amd64/list.json"

[vyper]
# When disabled, vyper related handlers are not available
enabled = true
# A directory where vyper compilers would be downloaded to
compilers_dir = "/tmp/vyper-compilers"
# List of available versions updates cron formatted schedule
refresh_versions_schedule = "0 0 * * * * *"

[vyper.fetcher.list]
# List of all availaable vyper compilers and information about them
list_url = "https://raw.githubusercontent.com/blockscout/solc-bin/main/vyper.list.json"

[sourcify]
# When disabled, sourcify related handlers are not available
enabled = true
# Sourcify API endpoint
api_url = "https://sourcify.dev/server/"
# Number of failing attempts the server makes to Sourcify API
verification_attempts = 3
# The maximum period (in seconds) the service is waiting for the Sourcify response
request_timeout = 10

[metrics]
# When disabled, metrics are not available
enabled = false
# IP address and port number metrics related endpoint should listen to
addr = "0.0.0.0:6060"
# A route at which metrics related endpoint is avaialable
route = "/metrics"

[jaeger]
# When disabled, jaeger tracing is not available
enabled = false
# An endpoint where jaeger collects all traces
agent_endpoint = "localhost:6831"
```

### Environment variables

Besides configuration file, one could use environment variables
to configure the service. If case of overlapping, those values
overwrites values from configuration file.
Variables have a hierarchical nature which
corresponds to the hierarchy in configuration file.
Double underscore (`__`) is used as a separator. All variables should use
`SMART_CONTRACT_VERIFIER` as a prefix.

All available options for configuration through environment variables could be found at
[config/base.env](./config/base.env)

# Api

Service supports 4 types of verification:

## Solidity Multi-Part files

### Route
`POST /api/v1/solidity/verify/multiple-files`

### Input

```json5
{
  // (optional) Creation transaction input.
  // If present, is used for contract verification,
  // otherwise deployed bytecode is used
  "creation_bytecode": "0x608060...0033000b0c",
  // Bytecode stored in the blockchain
  "deployed_bytecode": "0x608060...0033",
  // Compiler version used to compile the contract
  "compiler_version": "v0.8.14+commit.80d49f37",
  // Contains a map from a source file name to the actual source code
  "sources": {
    "A.sol": "pragma solidity ^0.8.14; contract A {}",
    "B.sol": "pragma solidity ^0.8.14; contract B {}"
  },
  // Version of the EVM to compile for
  "evm_version": "default",
  // If present, optimizations are enabled with specified number of runs, 
  // otherwise optmimizations are disabled
  "optimization_runs": 200,
  // If present, specify addresses of the libraries.
  "contract_libraries": {
    "MyLib": "0x123123..."
  }
}
```

## Solidity Standard-JSON input

### Route
`POST /api/v1/solidity/verify/standard-json`

### Input
```json5
{
  // (optional) Creation transaction input.
  // If present, is used for contract verification,
  // otherwise deployed bytecode is used
  "creation_bytecode": "0x608060...0033000b0c",
  // Bytecode stored in the blockchain
  "deployed_bytecode": "0x608060...0033",
  // Compiler version used to compile the contract
  "compiler_version": "v0.8.14+commit.80d49f37",
  // https://docs.soliditylang.org/en/latest/using-the-compiler.html#input-description
  "input": "{\"language\": \"Solidity\",\"sources\": { ... }, \"settings\": { ... }}"
}
```

## Sourcify
Proxies verification requests to Sourcify service and returns responses (https://docs.sourcify.dev/docs/api/server/v1/verify/).

### Route
`POST /api/v1/sourcify/verify`

### Input
```json5
{
  // Address of the contract to be verified 
  "address": "0xcafecafecafecafecafecafecafecafecafecafe",
  // The chain (network) the contract was deployed to 
  // (https://docs.sourcify.dev/docs/api/chains/)
  "chain": "100",
  // Files required for verification (see Sourcify Api)
  "files": {
    "A.sol": "pragma solidity ^0.8.14; contract A {}",
    "B.sol": "pragma solidity ^0.8.14; contract B {}",
    // https://docs.soliditylang.org/en/v0.8.14/metadata.html
    "metadata.json": "{ ... }"
  },
  // (optional) see Sourcify Api
  "chosenContract": 1
}
```

## Vyper Multi-Part files

### Route
`POST /api/v1/vyper/verify/multiple-files`

### Input
```json5
{
  // (optional) Creation transaction input.
  // If present, is used for contract verification,
  // otherwise deployed bytecode is used
  "creation_bytecode": "0x608060...0033000b0c",
  // Bytecode stored in the blockchain
  "deployed_bytecode": "0x608060...0033",
  // Compiler version used to compile the contract
  "compiler_version": "0.3.6+commit.4a2124d0",
  // Contains a map from a source file name to the actual source code
  "sources": {
    "A.vy": "# @version ^0.3.6\r\n\r\nuserName: public(String[100])\r\n\r\n@external\r\ndef __init__(name: String[100]):\r\n    self.userName = name\r\n\r\n@view\r\n@external\r\ndef getUserName() -> String[100]:\r\n    return self.userName\r\n"
  },
  // Version of the EVM to compile for
  "evm_version": "istanbul"
}
```

## Outputs
All verification requests have the same response format.

### Success
If verification succeeds, the service returns 200 with a success status:
```json5
{
  "message": "OK",
  "result": {
    // The name of the file verified contract was located at 
    "file_name": "A.sol",
    // The name of the contract which was verified
    "contract_name": "A",
    // Compiler version used to compile the contract
    "compiler_version": "v0.8.14+commit.80d49f37",
    // Source files given for verification
    "sources": {
      "A.sol": "pragma solidity ^0.8.14; contract A {}",
      "B.sol": "pragma solidity ^0.8.14; contract B {}"
    },
    // Version of the EVM contract was compile for
    "evm_version": "default",
    // (optional) WARNING: Before version 0.8.6 omitting the 'enabled' key was not equivalent to setting
    // it to false and would actually disable all the optimizations.
    "optimization": true,
    // (optional) Specify number of optimizer runs, if optimizations are enabled
    "optimization_runs": 200,
    // Addresses of the libraries
    "contract_libraries": {
      "MyLib": "0x123123..."
    },
    // Raw settings pushed submitted to the compiler on local compilation
    // (https://docs.soliditylang.org/en/v0.8.17/using-the-compiler.html#input-description)
    "compiler_settings": "{ ... }",
    // (optional) automatically extracted from creation transaction input
    // constructor arguments used for deploying verified contract
    "constructor_arguments": "0xcafecafecafe",
    // (optional) contract abi (https://docs.soliditylang.org/en/latest/abi-spec.html?highlight=abi#json);
    // is `null` for Yul contracts
    "abi": "[ { ... } ]",
    // (optional) creation transaction input resultant from local compilation
    // parsed and split on Main and Meta parts. Is null for Sourcify verification.
    "local_creation_input_parts": [
      { "type": "main", "data": "0x1234.." },
      { "type": "meta", "data": "0xcafe.." }
    ],
    // (optional) deployed bytecode resultant from local compilation
    // parsed and split on Main and Meta parts. Is null for Sourcify verification.
    "local_deployed_bytecode_parts": [
      { "type": "main", "data": "0x1234.." },
      { "type": "meta", "data": "0xcafe.." }
    ]
  },
  // Status of "0" indicates successful verification
  "status": "0"
}
```

### Verification Failure
If verification fails because of invalid verification data provided to it from outside,
the service returns 200 with the failure status:
```json5
{
  // Message indicating the reason for failure
  "message": "Compilation error: contracts/3_Ballot.sol:4:1: ParserError: Expected pragma, import directive or contract/interface/library/struct/enum/constant/function definition.\n12312313vddfvfdvfd\n^------^",
  // Non-zero status indicates an error code (currently only error code of `1` is possible)
  "status": "1"
}
```

### Bad Request
However, there are data that the requester is responsible for ensuring their validity.
Currently, it is related only to the creation of transaction input and deployed bytecode
stored in the chain for the contract to be verified, and the compiler version used in verification.

In case any of that arguments are invalid, the service return 400 BadRequest error,
indicating that something is wrong with the caller.

## Version List

### Route
`GET /api/v1/solidity/versions`

### Input
No input required

### Output

```json5
{
  // List of all available versions in descending order
  "versions": ["0.8.15-nightly.2022.5.27+commit.095cc647","0.8.15-nightly.2022.5.25+commit.fdc3c8ee",..]
}
```

### Route
`GET /api/v1/vyper/versions`

### Input
No input required

### Output

```json5
{
  // List of all available versions in descending order
  "versions": ["v0.3.6+commit.4a2124d0","v0.3.4+commit.f31f0ec4",..]
}
```
