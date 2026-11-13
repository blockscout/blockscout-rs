# Blockscout ENS service

This project provides indexed data of domain name service for blockscout instances.

Here is brief overview of the project structure:

![bens-structure](images/bens.drawio.svg)

Service is **multi-chain**, meaning that only one instance of `graph-node`, `postgres` and `bens-server` is required.

## Current supported domains

| Subgraph Name       | Network   | TLD   | Note |
|---------------------|-----------|-------|------|
| ens-subgraph        | Ethereum  | .eth  | ENSv1 + ENSv2, see [ENSv2](#ensv2) |
| rns-subgraph        | Rootstock | .rsk  |      |
| genome-subgraph     | Gnosis    | .gno  | SpaceID contracts |
| bns-subgraph        | Base      | .base |      |
| mode-subgraph       | Mode      | .mode | SpaceID contracts |
| lightlink-subgraph  | Lightlink | .ll   | SpaceID contracts |
| zns-subgraph        | Polygon   | .poly |      |
| d3-connect-subgraph | Shibarium | .shib |      |
| story-subgraph      | Story     | .ip   | SpaceID contracts |
| gravity-subgraph    | Gravity   | .g    | SpaceID contracts |
| zeta-subgraph       | Zeta      | .zeta | SpaceID contracts |

## ENSv2

`ens-subgraph` is [ensdomains/ens-subgraph](https://github.com/ensdomains/ens-subgraph)
`v2.3.1` with Blockscout's extension fields (see its
[README](./graph-node/subgraphs/ens-subgraph/README.md)). It indexes ENSv1 and
ENSv2 on mainnet and Sepolia and projects ENSv2 names into the legacy `Domain`
tables, so BENS keeps working on the same schema. Since v2.2.0 those rows are
correct on their own: the ENSv1 -> ENSv2 migration and the Graveyard's
retirement of expired names hand ENSv1 state to the Graveyard or a migration
controller, but the subgraph never stores one of those contracts as `owner`,
`registrant` or `wrappedOwner`. A migrated or natively registered ENSv2 name
carries its ENSv2 holder, a retired `.eth` name keeps its lapsed holder and
expiry (it reads as expired), and a retired subname is deleted with the ENSv1
convention (owner = zero address, reported as no owner). BENS needs no
ownership rules of its own. Specific to this subgraph:

- **Graph Node `v0.45.0`+** is required (`graph-node/docker-compose.yml`): the
  manifest uses `indexerHints` and `apiVersion 0.0.9`, which `v0.34.x` rejects.
  The bundled manifest is `specVersion 1.2.0`, not upstream's `1.3.0`, so that
  `vid` keeps its sequence default and BENS's offchain `domain` inserts work
  (see `graph-node/subgraphs/ens-subgraph/README.md`).
- **`specific.ensv2`.** `"ensv2": true` on an ENS-like protocol (default
  `false`) marks its subgraph as ens-subgraph 2.3+ and enables the ENSv2
  columns and tables below. Protocols without it never touch them.
- **Deploy order.** At startup BENS reads the schema of the *newest* deployment
  of each subgraph name (`subgraph_version` ordered by
  `deployment_schemas.created_at`), not graph-node's `current_version`, and
  whether or not it has synced. Deploy the new subgraph version, wait until it
  is fully synced, then start BENS with `"ensv2": true`. Any BENS restart while
  the new deployment is still syncing serves its partial data.
- **ENSIP-19 default primary names.** The ENSv2 app sets a primary name on the
  `DefaultReverseRegistrar` instead of the v1 `addr.reverse` resolver. The
  subgraph indexes them as `ENSv2PrimaryName`, with a Blockscout extension
  field `reverse_node = namehash(<addr>.addr.reverse)`. BENS's
  `addr_reverse_names` view UNIONs that table with the v1 `name_changed`
  source and follows ENSIP-19: the `addr.reverse` name wins, the default
  primary name is the fallback when it is empty or absent.
- **ENSv2 tokens.** For a name live in ENSv2 the subgraph sets
  `Domain.registryToken` (ens-subgraph 2.3+) to its current ENSv2 token,
  whose id is the registry address followed by the uint256 token id. BENS
  links that token instead of the ENSv1 BaseRegistrar or NameWrapper token,
  which is retired (held by the Graveyard after a migration) or unrelated (a
  native registration of a label whose ENSv1 registration had lapsed).
- **Never-expiring names.** The subgraph writes `253402300799`
  (9999-12-31T23:59:59Z) to `expiry_date` for ENSv2 names that never expire.
  BENS reports that value, or anything beyond it, as no expiry
  (`expiry_date: null`, "never expires").

```json
"specific": {
  "type": "ens_like",
  "native_token_contract": "0x57f1887a8BF19b14fC0dF6Fd9B2acc9Af147eA85",
  "registry_contract": "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e",
  "try_offchain_resolve": true,
  "ensv2": true
}
```

## Envs

[anchor]: <> (anchors.envs.start.envs_main)

| Variable                                         | Req&#x200B;uir&#x200B;ed | Description                                                   | Default value  |
| ------------------------------------------------ | ------------------------ | ------------------------------------------------------------- | -------------- |
| `BENS__DATABASE__CONNECT__URL`                   | true                     | e.g. `postgresql://postgres:postgres@localhost:5432/postgres` |                |
| `BENS__DATABASE__CREATE_DATABASE`                |                          |                                                               | `false`        |
| `BENS__DATABASE__RUN_MIGRATIONS`                 |                          |                                                               | `false`        |
| `BENS__SERVER__HTTP__ADDR`                       |                          |                                                               | `0.0.0.0:8050` |
| `BENS__SERVER__HTTP__ENABLED`                    |                          |                                                               | `true`         |
| `BENS__SERVER__HTTP__MAX_BODY_SIZE`              |                          |                                                               | `2097152`      |
| `BENS__SUBGRAPHS_READER__REFRESH_CACHE_SCHEDULE` |                          |                                                               | `0 0 * * * *`  |
| `BENS__TRACING__ENABLED`                         |                          |                                                               | `true`         |
| `BENS__TRACING__FORMAT`                          |                          |                                                               | `default`      |
| `BENS__SUBGRAPHS_READER__REFRESH_CACHE_DISABLED` |                          |                                                               | `false`        |
| `BENS__SUBGRAPHS_READER__MAX_PROTOCOLS_FROM_USER_INPUT` |                    | max protocols per request when chain id is omitted; `null` disables the limit | `5`            |
| `BENS__REPLICA_DATABASE__CONNECT__URL`           | true                     | e.g. `postgresql://postgres:postgres@localhost:5433/postgres` |                |
| `BENS__REPLICA_DATABASE__HEALTH_CHECK_INTERVAL`  |                          |                                                               | `10`           |
| `BENS__REPLICA_DATABASE__MAX_LAG`                |                          |                                                               | `300`          |

[anchor]: <> (anchors.envs.end.envs_main)

### Read replica

`BENS__REPLICA_DATABASE__*` is optional. When it is configured, read queries are served by the
replica while it is healthy (reachable and lagging no more than `MAX_LAG` seconds behind, checked
every `HEALTH_CHECK_INTERVAL` seconds), and by the main database otherwise.

A replica that is missing, unreachable or misconfigured at startup never blocks the start of the
service: BENS logs a warning and serves all reads from the main database. Note that the replica
stays disabled until the service is restarted, so restart BENS once the replica is back.

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
    - New directory inside `blockscout-ens/graph-node/subgraphs` with your subgraph code
    - Updated BENS config
    - Updated supported domains list
    - Result of indexed data: proof that your indexed subgraph contains correct amount of domains, resolved_addresses and so on