# Graph-node

`graph-node` indexes events in ethereum blockchain

One can submit subgraph to `graph-node` -- actual code how to handle new events of contracts

## Start locally using docker-compose

1. Edit `docker-compose.yml` and change `ethereum` ENV variable of `graph-node` services to add your own network and RPC url.

1. Start your own graph-node with docker-compose:

    ```bash
    docker-compose up -d
    ```

1. Load small version of ens-rainbow

    ```bash
    ./rainbow.small.sh
    ```

    Or use [full ens-rainbow](https://github.com/graphprotocol/ens-rainbow/) dump if you want full domain name resolving

1. Read [subgraphs/README.md](./subgraphs/README.md) to build and deploy subgraph to graph-node
