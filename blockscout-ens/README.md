# Blockscout ENS service

This project provides indexed data of domain name service for blockscout instances.

Here is brief overview of the project stucture:

![bens-structure](images/bens.drawio.svg)

Service is **multi-chain**, meaning that only one instance of `graph-node`, `postgres` and `bens-server` is required.

## Contribute

If you want to add your name service procol to blockscout you should:

1. Write subraph code: [Subgraph writer](./graph-node/subgraph-writer/README.md#howto-create-subgraph-for-your-domain-name-protocol)
2. Start graph-node: [Graph node](./graph-node/README.md#start-locally-using-docker-compose)
3. Deploy subgraph to graph-node: [Subgraphs: deploy](./graph-node/subgraphs/README.md#deploy-subgraph-to-graph-node)
4. Start `bens-server` connected to common database: [Bens server: start locally](./bens-server/README.md#to-start-locally)
