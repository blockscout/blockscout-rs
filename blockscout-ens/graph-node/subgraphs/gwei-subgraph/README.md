# Gwei Name Service subgraph

This subgraph indexes `.gwei` names registered through the Gwei Name Service
`NameNFT` contract on Ethereum and Sepolia.

GNS uses ENS namehashes as ERC-721 token IDs. The contract is also the resolver:
an active name resolves to its explicit `setAddr` value when nonzero and otherwise
falls back to its current owner. `PrimaryNameSet` selects reverse names, which are
only exposed when forward resolution still matches the selected address.

The mapping indexes registrations, renewals, subdomains, transfers, native and
multicoin addresses, content hashes, and primary-name selections. Resolver
entities are versioned per registration so records from an expired registration
cannot leak into a later registration of the same name.

## Networks

| Network | Contract | Start block |
|---------|----------|-------------|
| Ethereum | `0x9D51D507BC7264d4fE8Ad1cf7Fe191933A0a81d6` | `25403689` |
| Sepolia | `0x9D51D507BC7264d4fE8Ad1cf7Fe191933A0a81d6` | `11142856` |

## Build

```bash
yarn install
yarn codegen
yarn build
yarn test
```
