# <h1 align="center">Ethereum Bytecode Database</h1>

The Ethereum Bytecode Database powers the Vera verification application 
([https://vera.blockscout.com/](https://vera.blockscout.com/)).

Vera is a standalone application designed for easy, universal, 
multi-chain smart contract verification. 
It allows users to verify their contracts on multiple chains and add them 
to the [Verifier Alliance](https://verifieralliance.org/who.html) (Vera) database.

## Requirements
- PostgreSQL database
- eth-bytecode-db

## How to Enable
The Vera application is standalone and can run independently. 
The corresponding frontend code is not yet open-sourced.

## Envs
Here, we describe variables specific to this service. Variables common to all services can be found [here](../docs/common-envs.md).

[anchor]: <> (anchors.envs.start)

| Variable                                       | Required | Description                                                        | Default value                                      |
|------------------------------------------------|----------|--------------------------------------------------------------------|----------------------------------------------------|
| `PROXY_VERIFIER__CHAINS_CONFIG`                |          | A path to json file with chain configurations                      | (empty)                                            |
| `PROXY_VERIFIER__ETH_BYTECODE_DB__HTTP_URL`    |          | HTTP URL to underlying eth-bytecode-db service                     | `https://eth-bytecode-db.services.blockscout.com/` |
| `PROXY_VERIFIER__ETH_BYTECODE_DB__MAX_RETRIES` |          | Number of attempts server makes to the service. Must be at least 1 | `3`                                                |
| `PROXY_VERIFIER__ETH_BYTECODE_DB__PROBE_URL`   |          | If true, will check that HTTP URL can be connected to on startup   | `false`                                            |
| `PROXY_VERIFIER__ETH_BYTECODE_DB__API_KEY`     | true     | An api-key authorized to make requests to eth-bytecode-db service  |                                             |

[anchor]: <> (anchors.envs.end)

### Chain configurations
Chains may be configured via json file (see an [example](./config/chains.json)). 
The order specified in that file will be used by frontend application.

Besides, it is possible to re-set values for each chain via environment 
variables with `PROXY_VERIFIER_CHAINS` prefix. 
That can be used, for example, to specify `SENSITIVE_API_KEY` values.
Notice, that when combined, all three fields should be set for all specified chains 
(either via json file or via env).

| Variable                                               | Required | Description                                                                 | Default value                                      |
|--------------------------------------------------------|----------|-----------------------------------------------------------------------------|----------------------------------------------------|
| `PROXY_VERIFIER_CHAINS__{chain_id}__NAME`              |          | Name of the chain to be displayed to the user                               | (empty)                                            |
| `PROXY_VERIFIER_CHAINS__{chain_id}__API_URL`           |          | An url to the chain blockscout instance (e.g., https://eth.blockscout.com/) | (empty)                                            |
| `PROXY_VERIFIER_CHAINS__{chain_id}__SENSITIVE_API_KEY` |          | `API_SENSITIVE_ENDPOINTS_KEY` value of the corresponding instance           | (empty)                                            |

## Links
- Demo - https://proxy-verifier.services.blockscout.com/
- [Swagger](https://blockscout.github.io/swaggers/services/proxy-verifier/index.html)
- [Packages](https://github.com/blockscout/blockscout-rs/pkgs/container/proxy-verifier)
- [Releases](https://github.com/blockscout/blockscout-rs/releases?q=proxy-verifier&expanded=true)