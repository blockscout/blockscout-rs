<h1 align="center">Blockscout Rust Services</h1>

[![smart-contract-verifier](https://github.com/blockscout/blockscout-rs/actions/workflows/smart-contract-verifier.yml/badge.svg?branch=main)](https://github.com/blockscout/blockscout-rs/actions) 
[![sig-provider](https://github.com/blockscout/blockscout-rs/actions/workflows/sig-provider.yml/badge.svg?branch=main)](https://github.com/blockscout/blockscout-rs/actions)
[![multichain-search](https://github.com/blockscout/blockscout-rs/actions/workflows/multichain-search.yml/badge.svg?branch=main)](https://github.com/blockscout/blockscout-rs/actions)
[![visualizer](https://github.com/blockscout/blockscout-rs/actions/workflows/visualizer.yml/badge.svg?branch=main)](https://github.com/blockscout/blockscout-rs/actions)


A set of services used by [Blockscout](https://blockscout.com/) blockchain explorer, written in Rust.

## Services

1. [smart-contract-verifier](smart-contract-verifier/) - provides API for ethereum contract verification written in Solidity and Vyper

2. [sig-provider](sig-provider/) - aggregator of ethereum signatures for transactions and events

3. [multichain-search](multichain-search/) - service for searching transactions, blocks, tokens, etc in all blockscout instances. Contains frontend and backend parts.

4. [visualizer](visualizer/) - service for evm visualization such as:
   1. Solidity contract visualization using [sol2uml](https://www.npmjs.com/package/sol2uml)

## Contributing

We appreciate your support. Before writing code and submiting pull request, please read contributing [instructions](CONTRIBUTING.md).


## License


This project is primarily distributed under the terms of the MIT license. See [LICENSE-MIT](LICENSE-MIT) for details.
