# HoodID Blockscout/BENS subgraph

This folder contains a Blockscout BENS-compatible subgraph for HoodID `.hood` names.

It maps HoodID contract events into Blockscout's ENS-style `Domain`, `Account`, `Resolver`, and `Registration` schema so Blockscout can display names on address pages and in name-service search.

## Files

```text
subgraph.yaml                         # The Graph manifest
schema.graphql                        # Blockscout ENS/BENS schema
abis/HoodNameRegistry.json            # HoodID ABI
src/hood-name-registry.ts             # Event mappings
```

## Before deployment

Update `subgraph.yaml`:

```yaml
network: hood                         # graph-node network name for Robinhood/Hood Chain
source:
  address: "0x..."                    # deployed HoodNameRegistry address
  startBlock: 123456                   # HoodNameRegistry deployment block
```

Do not deploy with the placeholder zero address.

## Build locally

```bash
cd /Users/rasta/hoodid-contracts/subgraph
npm install
npm run codegen
npm run build
```

Verified locally with:

```text
Types generated successfully
Build completed: build/subgraph.yaml
```

## Deploy to graph-node

Example local graph-node deployment:

```bash
npm run deploy:local
```

Or against a hosted/custom graph-node:

```bash
cp .env.example .env
# Fill GRAPH_NODE_ADMIN_URL and GRAPH_NODE_IPFS_URL
npm run deploy
```

The deploy script runs:

```bash
graph codegen
graph build
graph create $SUBGRAPH_NAME --node $GRAPH_NODE_ADMIN_URL
graph deploy $SUBGRAPH_NAME --node $GRAPH_NODE_ADMIN_URL --ipfs $GRAPH_NODE_IPFS_URL --network $GRAPH_NODE_NETWORK_NAME --version-label $GRAPH_NODE_VERSION_LABEL
```

Manual equivalent:

```bash
graph create hoodid/hoodid-bens --node <GRAPH_NODE_ADMIN_URL>
graph deploy hoodid/hoodid-bens --node <GRAPH_NODE_ADMIN_URL> --ipfs <IPFS_URL> --network hood --version-label 0.0.1
```

## Blockscout operator config

After the subgraph is running, the Blockscout operator needs to enable BENS:

```bash
MICROSERVICE_BENS_ENABLED=true
MICROSERVICE_BENS_URL=<bens-service-url>
```

Then BENS points at the HoodID subgraph endpoint.

## Event mapping

| HoodID event | BENS entities updated |
|---|---|
| `BensNameRegistered` | `Domain`, `Account`, `Resolver`, `Registration`, `Transfer`, `NameRegistered`, `AddrChanged` |
| `BensNameRenewed` | `Domain.expiryDate`, `Registration.expiryDate`, `NameRenewed` |
| `BensResolverUpdated` | `Domain.resolvedAddress`, `Resolver`, `NewResolver`, `AddrChanged` |
| `BensNameTransferred` | `Domain.owner`, `Registration.registrant`, `Transfer`, `NameTransferred` |
| `BensPrimaryNameSet` | Refreshes `Domain.resolvedAddress` for wallet/name display |
