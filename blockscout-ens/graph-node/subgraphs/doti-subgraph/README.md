<h1 align="center">Blockscout Rust Services</h1>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/bens.yml">
<img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/bens.yml?branch=main&label=blockscout-ens&logo=github&style=flat-square"><!--
--></a>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/da-indexer.yml">
<img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/da-indexer.yml?branch=main&label=da-indexer&logo=github&style=flat-square"><!--
--></a>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/eth-bytecode-db.yml">
<img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/eth-bytecode-db.yml?branch=main&label=eth-bytecode-db&logo=github&style=flat-square"><!--
--></a>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/proxy-verifier.yml">
<img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/proxy-verifier.yml?branch=main&label=proxy-verifier&logo=github&style=flat-square"><!--
--></a>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/sig-provider.yml">
<img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/sig-provider.yml?branch=main&label=sig-provider&logo=github&style=flat-square"><!--
--></a>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/smart-contract-verifier.yml">
<img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/smart-contract-verifier.yml?branch=main&label=smart-contract-verifier&logo=github&style=flat-square"><!--
--></a>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/stats.yml">
<img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/stats.yml?branch=main&label=stats&logo=github&style=flat-square"><!--
--></a>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/tac-operation-lifecycle.yml">
<img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/tac-operation-lifecycle.yml?branch=main&label=tac-operation-lifecycle&logo=github&style=flat-square"><!--
--></a>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/user-ops-indexer.yml">
<img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/user-ops-indexer.yml?branch=main&label=user-ops-indexer&logo=github&style=flat-square"><!--
--></a>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/visualizer.yml">
<img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/visualizer.yml?branch=main&label=visualizer&logo=github&style=flat-square"><!--
--></a>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/multichain-search.yml">
<img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/multichain-search.yml?branch=main&label=multichain-search&logo=github&style=flat-square"><!--
--></a>
<a href="https://github.com/blockscout/blockscout-rs/actions/workflows/libs.yml">
<img src="https://img.shields.io/github/actions/workflow/status/blockscout/blockscout-rs/libs.yml?branch=main&label=libs&logo=github&style=flat-square"><!--
--></a>
A set of services used by Blockscout blockchain explorer, written in Rust.
Services
blockscout-ens - indexed data of domain name service for blockscout instances.
da-indexer - collects blobs from different DA solutions (e.g, Celestia)
eth-bytecode-db - Ethereum Bytecode Database. Cross-chain smart-contracts database used for automatic contracts verification
interchain-indexer - Universal Bridge Indexer. A standalone service for indexing and aggregating interoperability data across multiple blockchains
proxy-verifier - backend for the standalone multi-chain verification service
sig-provider - aggregator of ethereum signatures for transactions and events
smart-contract-verifier - smart-contracts verification
stats - service designed to calculate and present statistical information from a Blockscout instance
tac-operation-lifecycle - indexing operations in TAC (Ton Application Chain)
user-ops-indexer - service designed to index, decode and serve user operations as per the ERC-4337 standard
visualizer - service for evm visualization such as:
Solidity contract visualization using sol2uml
multichain-aggregator - service to store and index data from multiple blockscout instances. Also contains logic with interop messages
Running and configuring services
Services are distributed as docker images. For each service, you can find information about packages and their recent releases
inside service directories.
You can configure your app by passing the necessary environment variables when starting the container.
Configuration variables common for all services can be found here.
See full list of ENVs and their description inside service directories.
code
Shell
docker run -p 8050:8050 --env-file <path-to-your-env-file> ghcr.io/blockscout/{service-name}:latest
Alternatively, you can build your own docker images or compile them directly from sources.
Some of such options are described in build.
Project Layouts
Most of the projects consist of 3 main parts:
{service-name}-proto - defines the gRPC proto file with all API related data.
Defines mapping HTTP/JSON requests and their parameters to those gRPC methods.
{service-name}-logic - the crate with the implementation of the main business logic.
Note: previously the logic crate was named as {service-name};
some services still use that convention
{service-name}-server - initialize the server using the defined API.
Using the methods from “{service-name}-logic” to handle incoming requests.
The separation on "logic" and "server" crates is designed to separate functional and transport layers.
For now, "server" crates contain gRPC and HTTP as the transport layer.
It was assumed, that users may want to implement their own APIs, for which the library with functional logic might be used.
Crates that require database connection may also have additional sea-orm-defined crates:
{service-name}-migration - contains migrations for the database
{service-name}-entity - contains the entity files generated from the schema
Contributing
We appreciate your support. Before writing code and submitting a pull request, please read contributing instructions.
License
This project is primarily distributed under the terms of the MIT license. See LICENSE-MIT for details.
