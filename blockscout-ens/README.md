# Blockscout ENS service

This project provides indexed data of domain name service for blockscout instances.

Here is brief overview of the project structure:

![bens-structure](images/bens.drawio.svg)

Service is **multi-chain**, meaning that only one instance of `graph-node`, `postgres` and `bens-server` is required.

## Current supported domains

| Subgraph Name | Network | TLD | Note |
|--------------|---------|-----|------|
| ens-subgraph | Ethereum | .eth |      |
| rns-subgraph | Rootstock | .rsk |      |
| genome-subgraph | Gnosis | .gno | SpaceID contracts |
| bns-subgraph | Base | .base |      |
| mode-subgraph | Mode | .mode | SpaceID contracts |
| lightlink-subgraph | Lightlink | .ll | SpaceID contracts |
| zns-subgraph | Polygon | .poly |      |
| d3-connect-subgraph | Shibarium | .shib |      |


## Envs

[anchor]: <> (anchors.envs.start.envs_main)

| Variable | Req&#x200B;uir&#x200B;ed | Description | Default value |
| --- | --- | --- | --- |
| `BENS__DATABASE__CONNECT__URL` | true | e.g. `postgresql://postgres:postgres@localhost:5432/postgres` | |
| `BENS__DATABASE__CREATE_DATABASE` | | | `false` |
| `BENS__DATABASE__RUN_MIGRATIONS` | | | `false` |
| `BENS__SERVER__HTTP__ADDR` | | | `0.0.0.0:8050` |
| `BENS__SERVER__HTTP__ENABLED` | | | `true` |
| `BENS__SERVER__HTTP__MAX_BODY_SIZE` | | | `2097152` |
| `BENS__SUBGRAPHS_READER__REFRESH_CACHE_SCHEDULE` | | | `0 0 * * * *` |
| `BENS__TRACING__ENABLED` | | | `true` |
| `BENS__TRACING__FORMAT` | | | `default` |

[anchor]: <> (anchors.envs.end.envs_main)

## Quickstart developer run

1. Install [just](https://github.com/casey/just), [dotenv-cli](https://www.npmjs.com/package/dotenv-cli)

2. Run commands:
    ```bash
    just graph-node-start
    just deploy-subgraph ens-sepolia
    just run-dev
    ```


## Contribute

If you want to add your name service procol to blockscout you should:

1. Clone this `blockscout-rs` repo to add new protocol.
2. Write subraph code: read [subgraph writer guide](./graph-node/subgraph-writer/README.md#howto-create-subgraph-for-your-domain-name-protocol)
3. [OPTIONAL] if your protocol is based on SpaceID, read [SpaceID integration](./graph-node/README.md#spaceid-integration) section.
4. Add your protocol to deployment config [config.json](./graph-node/deployer/config.json)
5. Start graph-node (more in [graph-node guide](./graph-node/README.md#start-locally-using-docker-compose)):

   ```bash
   just graph-node-start
   ```

6. Deploy subgraph to graph-node (read more in [how to deploy subgraphs guide](./graph-node/README.md#deploy-subgraph-to-graph-node))
    ```bash
    just deploy-subgraph <protocol_name>
    ```

7. Add protocol to [dev.json](./bens-server/config/dev.json) config and start `bens-server` connected to common database (read more in [how to start bens guide](./bens-server/README.md#to-start-locally))

    ```bash
    just run-dev
    ```

8. Check that `bens-server` responses with valid domains. You can find swagger docs at [https://blockscout.github.io/swaggers/services/bens/main/index.html](https://blockscout.github.io/swaggers/services/bens/main/index.html)

9. Add your protocol to list of [supported domains](#current-supported-domains)

10. Update default config of BENS server for [production](./bens-server/config/prod.json) and [staging](./bens-server/config/staging.json)

11. Finally, create PR with:
    * New directory inside `blockscout-ens/graph-node/subgraphs` with your subgraph code
    * Updated BENS config
    * Updated supported domains list
    * Result of indexed data: proof that your indexed subgraph contains correct amount of domains, resolved_addresses and so on

