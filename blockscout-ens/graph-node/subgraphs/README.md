# Domains subgraph

## Current supported domains

+ `ens-subgraph`: Ethereum (.eth)
+ `rns-subgraph`: Rootstock (.rsk)
+ `genome-subgraph`: Gnosis (.gno)

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
