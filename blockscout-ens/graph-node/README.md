# Graph-node

`graph-node` indexes events in ethereum blockchain

One can submit subgraph to `graph-node` -- actual code how to handle new events of contracts

## Start locally using docker-compose

1. You may need to add your custom network to `config.toml`

2. Start your own graph-node with docker-compose:

    ```bash
    docker-compose up -d
    ```

3. Load small version of ens-rainbow

    ```bash
    ./rainbow.small.sh
    ```

    Or use [full ens-rainbow](https://github.com/graphprotocol/ens-rainbow/) dump if you want full domain name resolving


## Add your own subgraph

Read guide [How to add new subgraph](./subgraph-writer/README.md)


## Deploy subgraph to graph-node


### Convenient `deployer` script with `just`

1. Add your protocol to [deployer/config.json](./deployer/config.json)

2. Run deployer script to deploy to local graph-node:

```bash
just deploy-subgraph <protocol_name>
```

3. Run deployer script to deploy to remote graph-node:

```bash
just deploy-subgraph --prod <protocol_name>
```

### Manually with `yarn`

1. Initially:

    ```bash
    cd subgraphs/<subgraph_directory>
    yarn install
    yarn codegen
    yarn build
    ```

2. **[FOR BLOCKSCOUT TEAM]** Make sure you have access to graph, for example using port forwarding to staging graph-node:

    ```bash
    kubectl port-forward -n graph-node svc/graph-node 8020:8020
    ```

    Or you can run your own `graph-node` using docker: read [start locally](#start-locally-using-docker-compose)

3. Create subgraph on graph-node

    ```bash
    yarn graph create --node http://127.0.0.1:8020 <subgraph_name>
    ```

4. Deploy subgraph to graph-node

    ```bash
    yarn graph deploy  --node http://127.0.0.1:8020 --ipfs http://127.0.0.1:5001 --network <network_name> --version-label 0.0.1 <subgraph_name>
    ```

### Check results

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

> Note that we selected from custom schema called `sgd1`. This schema is created by `graph-node` and will be automatically incremented to `sgd2`, `sgd3`, etc.


+ Run `bens-server` API and send requests to check results of subgraph: read [bens-server docs](../../bens-server/README.md)


## SpaceID integration

Developing subgraph for protocol based on space-id contracts requires providing additional information.

SpaceID protocol has constant variable called `identifier` which unique describes protocol across multiple chains.
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
