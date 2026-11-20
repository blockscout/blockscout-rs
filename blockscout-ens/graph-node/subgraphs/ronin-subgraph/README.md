# Ronin Name Service subgraph

Indexes the Ronin Name Service (RNS, `.ron` names) for BENS on Ronin mainnet
(chain 2020) and Saigon testnet (chain 202601). Contract sources:
[axieinfinity/rns-contracts](https://github.com/axieinfinity/rns-contracts).
The schema is the Blockscout ENS schema with the `PrimaryNameRecord` table from
etn-subgraph, plus `BackfillState`.

## How RNS differs from ENS

- One contract, `RNSUnified`, is both the registry and the ERC-721: one token per
  name, and the token id is the namehash. Instead of the ENS registry events it
  emits `RecordUpdated(node, indicator, record)`; the indicator bits say which
  fields changed. Only `mint()` carries the label, so names are never hashed and
  no rainbow table is needed. Only second-level names and `<addr>.addr.reverse`
  nodes can be minted. The subgraph stores the token id in `Domain.tokenId`,
  which BENS uses for the NFT link instead of the ENS labelhash.
- `PublicResolver.setAddr` always reverts and `addr(node)` returns
  `RNSUnified.ownerOf(node)`, so `AddrChanged` is never emitted. Here
  `Domain.resolvedAddress` is the NFT owner of a name that has a resolver.
- `PublicResolver.name(<addr>.addr.reverse)` returns the stored name only while
  the address owns that name. BENS reads primary names from
  `primary_name_record`, which the mappings rebuild whenever a reverse name, a
  reverse node's resolver or a name's owner changes.
- No NameWrapper, no multicoin addresses, and the content hash event is spelled
  `ContentHashChanged`. `ReverseClaimed(address,uint256)` is not indexed: the
  address is the reverse node's label.
- Names owned by an account with `RESERVATION_ROLE` never expire. On mainnet the
  RNS auction contract holds about 135k of the ~191k tokens that way, without a
  resolver. Expiry is read with `getRecord`, which applies the role.

## Names minted before the L2 genesis

Ronin became an OP Stack L2 on 2026-05-12, and the RPC serves no block before the
L2 genesis (mainnet block 55,577,500, Saigon block 45,528,550). RNS launched on
mainnet at block 29,142,341, so the events of almost every name are out of reach.
The contract state was migrated, so the subgraph loads it from chain:

- The data sources start one block after the genesis, because graph-node reads
  `startBlock - 1` when it creates the deployment.
- `RNSUnifiedBackfill` is a polling block handler limited by `endBlock`. It walks
  `tokenByIndex` from the top index down and loads each token with `getRecord`,
  plus `name()` for reverse nodes. Walking down and clamping to the current
  supply means burns during the window cannot make it skip a token. Each token
  costs up to three `eth_call`s and graph-node allows about 2,000 per handler,
  so a block loads 400 tokens; the 1,000-block window holds 400k tokens.
- An event for a name the backfill has not reached yet loads that name the same
  way first.

This needs an **archive** Ronin RPC from the L2 genesis, since every `eth_call`
runs at the block being indexed. The public RPC caps `eth_getLogs` at 200 blocks
and is not meant for production use.

## Known limits

- Primary names are not re-checked when a name expires: the row stays until the
  name's next event (as in etn-subgraph).
- The backfill reads reverse names through `name()`, which only returns a name
  the address owns at that block. A reverse name pointing at a name the address
  did not hold at the L2 genesis is not seen until it is set again.
- A `RESERVATION_ROLE` grant or revoke after the backfill shows on each affected
  name at its next event only.
- Names created before the L2 genesis have the genesis time as `createdAt` and
  no registration until their next renewal.

## Files

- `abis/`: ABIs of the logic contracts in rns-contracts
  `deployments/ronin-mainnet` (the Saigon ones have the same signatures), with
  the struct field `protected` renamed to `isProtected` because AssemblyScript
  reserves the word. Parameter names are not part of event or function
  signatures.
- `networks.json`: proxy addresses from
  [axieinfinity/smart-contract-tracker](https://github.com/axieinfinity/smart-contract-tracker)
  `contracts/rns/rns.json`; `subgraph.yaml` is written from it by
  `graph build --network`.

## Build and test

```bash
yarn install
yarn codegen
yarn build:mainnet   # or yarn build:saigon
yarn test
```

## Deploy

`../../deployer/config.json` has `ronin-name-service` and
`ronin-name-service-saigon`. graph-node needs networks named `ronin-mainnet` and
`ronin-saigon`, each backed by an archive RPC.
