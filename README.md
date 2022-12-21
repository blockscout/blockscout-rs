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

1. [smart-contract-verifier](smart-contract-verifier/) - provides API for ethereum contract verification written in Solidity and Vyper

2. [sig-provider](sig-provider/) - aggregator of ethereum signatures for transactions and events

3. [multichain-search](multichain-search/) - service for searching transactions, blocks, tokens, etc in all blockscout instances. Contains frontend and backend parts.

4. [visualizer](visualizer/) - service for evm visualization such as:
   1. Solidity contract visualization using [sol2uml](https://www.npmjs.com/package/sol2uml)

5. [eth-bytecode-db](eth-bytecode-db/) - Ethereum Bytecode Database. Verifies smart-contracts and searches for the sources via bytecodes  

## Contributing

We appreciate your support. Before writing code and submiting pull request, please read contributing [instructions](CONTRIBUTING.md).


## License


This project is primarily distributed under the terms of the MIT license. See [LICENSE-MIT](LICENSE-MIT) for details.
