
# Howto create subgraph for your domain name protocol

The first thing to note is that the closer your protocol is to **ENS**, the easier it will be to create a blockscout-compatible subgraph.
We take initial structure from [ENS subgraph](https://github.com/ensdomains/ens-subgraph).
You can take a look at that subgraph and understand structure of our project more precisely.

1. Install [just](https://github.com/casey/just). `Just` is like cmake but better.

1. Install python3 and install deps:
  
    ```bash
    # running in this directory
    just init
    ```

1. Now you have to create file inside `protocols` directory decribing your procol. Use `example.protocol.yaml` as template.

    **[EXPEREMENTAL]** You can try to generate protocol desription file using `protocol-extractor`. This script will try to extract verified   contracts from etherscan and determine their affiliation with the protocol:

    ```bash
    just try-build-protocol <protocol-name> <etherscan-endpoint-with-api-key> <addresses-of-contracts-comma-separated>
    ```

    This command will create `protocols/<name>.yaml` file with decription of contracts. You still need to add `project_name` and other metadata field. Also change generated fields it if necessary.

1. Generate subgraph project from template:

    ```bash
    just template-it-from-protocol protocols/<name>.yaml ../subgraphs
    ```

    This command will create project inside `../subgraphs/<project_name>`

1. Move to recently created directory and run

    ```bash
    yarn init && yarn codegen
    ```

    In case of any error, adjust typescript code of subgraph. Also make sure subgraph handles events properly.

1. Write your mappings: read [official subgraph guide](https://thegraph.com/docs/en/developing/creating-a-subgraph/#writing-mappings). You have to handle events of your protocol properly in order to index all blockchain data. You can use default mapping from generated template, however make sure that code is written correctly.

1. Run default tests that will check name hashing logic

    ```bash
    # you can ommit -d flag to run tests without docker, but in case of MacOS we suggest you to use docker
    yarn graph test -d
    ```

1. Now build subgraph code
  
    ```bash
    yarn build
    ```

1. Now you should run your subgraph by submitting it to graph-node: read [deploy subgraphs to graph-node](../README.md#deploy-subgraph-to-graph-node)

