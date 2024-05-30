# Domains subgraph

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

## Add your own subgraph

Read guide [How to add new subgraph](../subgraph-writer/README.md)

## Deploy subgraph to graph-node

> For every directory, it's posibble to deploy subgraph to blockscout graph-node

1. Initially:

    ```bash
    cd <subgraph_directory>
    just init
    just codegen
    just build
    ```

1. Make sure you have access to graph, for example using port forwarding to staging graph-node (for blockscout dev):

    ```bash
    kubectl port-forward -n graph-node svc/graph-node 8020:8020
    ```

    Or you can run your own `graph-node` using docker: read [graph-node: start locally](../README.md#start-locally)

1. Create subgraph on graph-node

    ```bash
    just create
    ```

1. Deploy subgraph to graph-node

    ```bash
    # deploy to blockscout graph-node
    just deploy-remote
    # deploy to local graph-node
    just network="network-name" deploy
    ```

1. Check errors and result:

   + Connect to subgraph database and check current state of deployed subgraph:

        ```postgres
        select deployment, failed, health, synced, latest_ethereum_block_number, fatal_error from subgraphs.subgraph_deployment;
        
        select * from subgraphs.subgraph_error;
        
        select name, resolved_address, expiry_date 
        from sgd1.domain 
        where label_name is not null and block_range @> 2147483647 
        order by created_at 
        limit 100;
        ```

   + Run `bens-server` API and send requests to check results of subgraph: read [bens-server docs](../../bens-server/README.md)

## SpaceID integration

Developing subgraph for protocol based on space-id contracts requires providing additional information.

SpaceID protocol has constant variable called `identifier` which unique describes protocol accross multiple chains.
This values is used during calculation of [namehash](https://docs.ens.domains/resolution/names#algorithm), therefore subgraph should know this value.

Actually blockscout-ens needs two values that can be calculated from `identifier`: `empty_label_hash` and `empty_label_hash`

+ `empty_label_hash` is basically hashed `identifier`
+ `base_node_hash` is hash of `base_node` plus `empty_label_hash`


### Obtaining `empty_label_hash` and `base_node_hash`

To obtain it, you need to make `eth_call` to Base contract (contract with NFT):

+ `mode-sepolia` example

    ```bash
    BASE_NODE=mode \
    RPC_URL=https://sepolia.mode.network \
    CONTRACT=0xCa3a57e014937C29526De98e4A8A334a7D04792b \
    python3 tools/fetch-space-id.py
    
    OUTPUTS:
    identifier:       '0x00000397771a7e69f683e17e0a875fa64daac091518ba318ceef13579652bd79'
    empty_label_hash: '0xea1eb1136f380e6643b69866632ce3b493100790c9c84416f2769d996a1c38b1'
    base_node_hash:   '0x9217c94fd014da21f5c43a1fcae4154a2bbfce43eb48bb33f7f6473c68ee16b6'
    ```

+ `mode-mainnet` example

    ```bash
    RPC_URL=https://mainnet.mode.network \
    CONTRACT=0x2ad86eeec513ac16804bb05310214c3fd496835b \
    BASE_NODE=mode \
    python3 tools/fetch-space-id.py

    OUTPUTS:
    identifier:       '0x0000868b771a7e69f683e17e0a875fa64daac091518ba318ceef13579652bd79'
    empty_label_hash: '0x2fd69f9e5bec9de9ebf3468dafc549eca0bc7d17dfbc09869c2cfc3997d5d038'
    base_node_hash:   '0x2f0e9a68fa134a18a7181045c3549d639665fe43df78e882d8adea865a4bb153'
    ```

### Using `empty_label_hash` and `base_node_hash`

1. `base_node_hash` for subgraph

Put `base_node_hash` (without `0x` prefix) inside `utils.ts`, for example:

```
export const BASE_NODE_HASH = "9217c94fd014da21f5c43a1fcae4154a2bbfce43eb48bb33f7f6473c68ee16b6"
```

Also don't forget to replace this value if you change network (from mainnet to testnet for example)

1. `empty_label_hash` for BENS

Put `empty_label_hash` in json configuration of BENS. 

Use mainnet inside [prod.json](../../bens-server/config/prod.json) and testnet [staging.json](../../bens-server/config/staging.json)
