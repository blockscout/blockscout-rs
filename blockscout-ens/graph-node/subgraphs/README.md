# Domains subgraph

## Current supported domains

+ `ens-subgraph`: Ethereum (.eth)
+ `rns-subgraph`: Rootstock (.rsk)

## To start

> For every directory, it's posibble to deploy subgraph to blockscout graph-node


1. Initially:

```bash
cd <subgraph_directory>
just init
just codegen
just build
```

2. Deploy to blockscout:

+ Make sure you have access to graph, for example using port forwarding to staging graph-node:

```bash
kubectl port-forward -n graph-node svc/graph-node 8020:8020
```

+ Create subgraph

```bash
just create
```

+ Push it to graph-node

```bash
just deploy-remote
```
