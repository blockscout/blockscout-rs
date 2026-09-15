# Docker-compose for Rust Blockscout services

This docker-compose configuration created for testing whole blockscout mictroservice architecture from scratch.
Feel free to change variables inside `./envs` to customize batch of microservices.

## Run all

```bash
docker-compose up -d
```

You can adjust versions of services using env variables like `SMART_CONTRACT_VERIFIER_DOCKER_TAG`.

For example:

```bash
export SMART_CONTRACT_VERIFIER_DOCKER_TAG=v1.6.0
export ETH_BYTECODE_DB_DOCKER_TAG=v1.4.4
docker-compose up -d
```

## Run one service

Alternatively, you can run one or several services, simply write:

```bash
docker-compose up -d eth-bytecode-db
```
