# <h1 align="center"> Verification </h1>

A contract verification service. Runs as an HTTP server and allows
making verification requests through REST API. It is stateless
and answers requests based on provided information only.

## Building from source
Install rustup from rustup.rs.
```
git clone git@github.com:blockscout/blockscout-rs.git

cd blockscout-rs

cargo build --all --release
```
You can find the built binary in `target/release` folder.

## Installing through cargo
Another way to install the binary without cloning the repository is to use cargo straightway:
```
cargo install --git https://github.com/blockscout/blockscout-rs --bin verification
```
In that case, you can run the binary using just `verification`.

## Configuration
Service uses a configuration file the path to which is specified via CLI flag `--config-path=[path]`.
The configuration file may contain the following options:
```toml
[server]
# IP address and port number the server should listen to
addr = "0.0.0.0:8043"

[solidity]
# when disabled, solidity related handlers are not available
enabled = true
# list of all available compilers and information about them
compilers_list_url = "https://raw.githubusercontent.com/blockscout/solc-bin/main/list.json"

[sourcify]
# when disabled, sourcify related handlers are not available 
enabled = true
# Sourcify API endpoint
api_url = "https://sourcify.dev/server/"
# number of failing attempts the server makes to Sourcify API 
verification_attempts = 3
# the maximum period (in seconds) the service is waiting for the Sourcify response
request_timeout = 10
```
For all keys omitted from the configuration file default values from the example above are used.

# Api

Service supports 4 types of verification:
## Single file
**Note**: Is deprecated and going to be replaced by Multi-Part files verification

### Route
`/api/v1/solidity/verify/flatten`

### Input

```json5
{
  // Creation transaction input
  "creation_bytecode": "0x608060...0033000b0c",
  // Bytecode stored in the blockchain
  "deployed_bytecode": "0x608060...0033",
  // Compiler version used to compile the contract
  "compiler_version": "v0.8.14+commit.80d49f37",
  // Source code
  "source_code": "pragma solidity ^0.8.14; ...",
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

## Multi-Part files
**Note**: currently WIP and is not available right now

### Route
`/api/v1/solidity/verify/multi-files`

### Input
The only difference with Single file input is that the `source_code` field was replaced by `sources` allowing to submit several files for verification.

```json5
{
  // Creation transaction input
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

## Standard-JSON input

### Route
`/api/v1/solidity/verify/standard-json`

### Input
```json5
{
  // Creation transaction input
  "creation_bytecode": "0x608060...0033000b0c",
  // Bytecode stored in the blockchain
  "deployed_bytecode": "0x608060...0033",
  // Compiler version used to compile the contract
  "compiler_version": "v0.8.14+commit.80d49f37",
  // https://docs.soliditylang.org/en/latest/using-the-compiler.html#input-description
  "input": {
    "language": "Solidity",
    "sources": { ... },
    "settings": { ... }
  }
}
```

## Sourcify
Proxies verification requests to Sourcify service and returns responses (https://docs.sourcify.dev/docs/api/server/v1/verify/).

### Route
`/api/v1/sourcify/verify`

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
    // (optional) automatically extracted from creation transaction input
    // constructor arguments used for deploying verified contract
    "constructor_arguments": "0xcafecafecafe",
    // (https://docs.soliditylang.org/en/latest/abi-spec.html?highlight=abi#json)
    "abi": "[ { ... } ]"
  },
  // Status of 0 indicates successful verification
  "status": 0
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
  "status": 1
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
`/api/v1/solidity/versions`

### Input
No input required

### Output

```json5
{
  // List of all available versions in descending order
  "builds": ["0.8.15-nightly.2022.5.27+commit.095cc647","0.8.15-nightly.2022.5.25+commit.fdc3c8ee",..]
}
```