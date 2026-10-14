# Rensa `.rns` BENS subgraph

This is a protocol-specific adapter over the immutable RNSRegistryV2 at `0x08ed77b2ec313c7ad5ce23747b07d483071485f9` on Robinhood Chain Mainnet (chain ID `4663`, graph-node network `robinhood`). Indexing begins at deployment block `70985779`.

Rensa is **not** an ERC-137 registry. ERC-721 `Transfer` events provide canonical ownership; Rensa-specific events provide human-readable labels, expiry, resolver updates, and primary-name changes. The BENS `Domain.id` is an ENS-style full-name namehash used solely as an adapter lookup key. `Domain.labelhash` and `Domain.tokenId` represent the real Rensa ERC-721 ID, `keccak256(bytes(label))`.

The `.rns` root is an index-only parent. Reserved names are reconstructed from constructor events, not hardcoded. `expiryDate` is the real onchain registration expiry; `graceEndsAt` is separate. BENS config enables a 7,776,000-second grace only for active domain/forward lookups. Primary-name reverse lookup still requires current ownership and normal active expiry, with no grace extension.

Run `yarn install`, `yarn codegen`, `yarn build`, and `yarn test` here. Do not deploy from this repository without Blockscout operator approval. `deployer/config.json` names this subgraph, but the operator must configure a historical Robinhood Chain `robinhood` provider in graph-node and enable the `rensa` BENS protocol. No external GraphQL hosting or private RPC credential is committed.
