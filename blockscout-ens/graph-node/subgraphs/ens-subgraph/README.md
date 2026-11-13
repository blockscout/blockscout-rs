# ENS Subgraph

> forked from [https://github.com/ensdomains/ens-subgraph](https://github.com/ensdomains/ens-subgraph)

This directory is the upstream subgraph at tag `v2.3.1` (commit `3628372`)
plus the Blockscout/BENS extension fields listed below. It indexes ENSv1
(registry, BaseRegistrar, controllers, NameWrapper, resolvers) and ENSv2
(RootRegistry, ETHRegistry, ETHRegistrar, user registries created through
VerifiableFactory, PermissionedResolver, the ENSIP-19
DefaultReverseRegistrar) on mainnet and Sepolia. ENSv2 names are projected
into the legacy `Domain` / `Registration` / `WrappedDomain` entities, so BENS
keeps reading the same tables; the native ENSv2 entities (`ENSv2Registry`,
`ENSv2NameSlot`, `ENSv2NamePath`, `ENSv2PrimaryName`, ...) sit next to them.
See `CHANGELOG.md` for the upstream history and `schema.graphql` for the
entity documentation.

## Blockscout/BENS extensions

Every change against upstream is marked `Blockscout/BENS extension` in the
source. They are:

- `Domain.storedOffchain: Boolean!`, `Domain.resolvedWithWildcard: Boolean!`,
  `Domain.tokenId: BigInt` (`schema.graphql`). BENS writes these for
  offchain-resolved names; the mappings default the booleans to `false` at
  every `Domain` construction site (`src/ensRegistry.ts`, `src/ensv2Domain.ts`,
  `src/utils.ts`) and in the test fixtures.
- `ENSv2PrimaryName.reverseNode: Bytes` (`schema.graphql`,
  `src/ensv2Reverse.ts`): `namehash(<address>.addr.reverse)` for the address
  of each ENSIP-19 default primary name, so BENS's `addr_reverse_names` view
  can join ENSv2 primary names to the v1 reverse-record keys without keccak
  in SQL.
- `specVersion: 1.2.0` in `subgraph.yaml` (upstream: `1.3.0`). From
  `1.3.0` graph-node enables strict `vid` ordering and creates every entity
  table's `vid` as a plain `bigint` with no default. BENS inserts offchain and
  wildcard `domain` rows without a `vid` (`bens-logic/src/subgraph/sql/create.rs`),
  which then fail. With `1.2.0` the column is a `bigserial`
  (`nextval('sgdN.domain_vid_seq')`). The manifest uses nothing else from
  `1.3.0` (no subgraph data sources), and `indexerHints` is valid in `1.2.0`.

To upgrade to a newer upstream tag, replace the directory with the new tree
(keep `justfile` and this file) and re-apply the three items above.

## Requirements

- **Graph Node `v0.45.0` or newer** (`../../docker-compose.yml`). The
  manifest is `specVersion 1.2.0` / `apiVersion 0.0.9` with `indexerHints`,
  which `v0.34.x` rejects.
- **`ens_names` must be seeded** before the first deployment. Graph Node's
  `ens.nameByHash` errors with "Missing ENS data" while `public.ens_names` is
  empty and the subgraph retries its first `NewOwner` forever.
  `../../rainbow.small.sh` loads the small set; load the full
  [ens-rainbow](https://github.com/graphprotocol/ens-rainbow/) dump for real
  ENSv1 subname labels. ENSv2 names do not need it: `LabelRegistered` carries
  the plain label.
- **An RPC that serves `eth_getLogs` without an address filter.** The
  `Resolver` data source is addressless. Public gateways rate-limit or refuse
  those calls; a keyed node is needed for a full sync.

## Deploy

Through the deployer (uses `networks.json` for the per-network addresses and
start blocks):

```bash
just deploy-subgraph ens          # mainnet,  subgraph name ens-subgraph
just deploy-subgraph ens-sepolia  # sepolia,  subgraph name ens-sepolia-subgraph
```

Or directly: `just network=sepolia name=ens-sepolia-subgraph deploy v2.3.1`.

## BENS configuration

The `ens` and `ens-sepolia` protocols in `bens-server/config/*.json` set
`specific.ensv2: true`. It makes BENS read the ENSv2 parts of this schema: the
reverse-name view includes ENSIP-19 default primary names, and a name live in
ENSv2 links its ENSv2 registry token (`Domain.registryToken`). See
`../../../README.md`, section "ENSv2".

## Tests

```bash
yarn install
yarn codegen
yarn test
```
