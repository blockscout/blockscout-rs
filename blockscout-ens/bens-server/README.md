# Bens-server

## To start locally

1. Compile and run:

    ```bash
    export BENS__DATABASE__CONNECT__URL="<database-url>"
    export BENS__BLOCKSCOUT__NETWORKS__<chain_id>__URL=<blockscout_url>
    cargo run --bin bens-server
    ```
