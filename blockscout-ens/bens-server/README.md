# Bens-server

## To start locally

1. Configuration
    1. Prepare env variables

        ```bash
        # graph-node database url
        BENS__DATABASE__CONNECT__URL=postgresql://graph-node:let-me-in@localhost:5432/graph-node?sslmode=disable
        # path to json config
        BENS__CONFIG=./config/dev.json
        ```

    1. You can change `dev.json` config and describe your protocols

1. Compile and run:

    ```bash
    # add env variables before
    cargo run --bin bens-server
    ```
