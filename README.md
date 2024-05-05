<h1 align="center">Blockscout Rust Services</h1>

<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/smart-contract-verifier.yml">
   <img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/smart-contract-verifier.yml?branch=main&label=smart-contract-verifier&logo=github&style=plastic">
</a> 
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/sig-provider.yml">
   <img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/sig-provider.yml?branch=main&label=sig-provider&logo=github&style=plastic">
</a> 
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/multichain-search.yml">
   <img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/multichain-search.yml?branch=main&label=multichain-search&logo=github&style=plastic">
</a> 
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/visualizer.yml">
   <img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/visualizer.yml?branch=main&label=visualizer&logo=github&style=plastic">
</a>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/eth-bytecode-db.yml">
   <img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/eth-bytecode-db.yml?branch=main&label=eth-bytecode-db&logo=github&style=plastic">
</a>


A set of services used by [Blockscout](https://blockscout.com/) blockchain explorer, written in Rust.

## Services

1. [blockscout-ens](blockscout-ens) - indexed data of domain name service for blockscout instances.

1. [da-indexer](da-indexer) - collects blobs from different DA solutions (e.g, Celestia) 

2. [eth-bytecode-db](eth-bytecode-db/) - Ethereum Bytecode Database. Cross-chain smart-contracts database used for automatic contracts verification

1. [proxy-verifier](proxy-verifier) - backend for the standalone multi-chain verification service

1. [scoutcloud](scoutcloud) - API to deploy and manage blockscout instances

2. [sig-provider](sig-provider/) - aggregator of ethereum signatures for transactions and events

3. [smart-contract-verifier](smart-contract-verifier/) - smart-contracts verification

1. [stats](stats) - service designed to calculate and present statistical information from a Blockscout instance

1. [user-ops-indexer](user-ops-indexer) - service designed to index, decode and serve user operations as per the ERC-4337 standard

4. [visualizer](visualizer/) - service for evm visualization such as:
   1. Solidity contract visualization using [sol2uml](https://www.npmjs.com/package/sol2uml)

## Running and configuring services

Services are distributed as docker images. For each service you can find information about packages and their recent releases
inside service directories.

You can configure your app by passing necessary environment variables when starting the container. 
Configuration variables common for all services can be found [here](docs/common-envs.md).
See full list of ENVs and their description inside service directories.

```shell
docker run -p 8050:8050 --env-file <path-to-your-env-file> ghcr.io/blockscout/{service-name}:latest 
```

Alternatively, you can build your own docker images or compile them directly from sources. 
Some of such options are described in [build](docs/build.md).

## Project Layouts

Most of the projects consist of 3 main parts:
1. `{service-name}-proto` - defines the gRPC proto file with all API related data.
   Defines mapping HTTP/JSON requests and their parameters to those gRPC methods.
2. `{service-name}-logic` - the crate with the implementation of the main business logic.
3. `{service-name}-server` - initialize the server using the defined API.
    Using the methods from “{service-name}-logic” to handle incoming requests.

The separation on "logic" and "server" crates is designed to separate functional and transport layers.
For now, "server" crates contain gRPC and HTTP as the transport layer. 
It was assumed, that users may want to implement their own APIs, for which the library with functional logic might be used.

Crates that require database connection may also have additional sea-orm defined crates:
1. `{service-name}-migration` - contains migrations for the database
2. `{service-name}-entity` - contains the entity files generated from the schema 

## Contributing

We appreciate your support. Before writing code and submiting pull request, please read contributing [instructions](CONTRIBUTING.md).


## License


This project is primarily distributed under the terms of the MIT license. See [LICENSE-MIT](LICENSE-MIT) for details.
