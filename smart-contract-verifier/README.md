# <h1 align="center"> Smart-contract Verifier </h1>

**Smart-contract verifier** is a service for verifying EVM-based contracts. The primary function of this service is to receive bytecode and potential source files as inputs and determine whether the files and the bytecode correspond to each other.

This service serves as the core component for all activities related to smart-contract verification in BlockScout. It is essential for enabling smart-contract verification functionality on your instance.

## Requirements
Production compiler endpoints require a dedicated Docker host reachable over SSH. Native execution
is intended only for local development and tests.

> **Upgrading:** compiler execution is now explicit. The Solidity, Vyper and zkSync endpoints are
> enabled by default, and the service refuses to start while
> `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__TYPE` is unset (`disabled`). Before upgrading an
> existing deployment, set it to `docker` together with the Docker settings below, or to `native`
> to keep the previous in-process behavior in local development.

## How to enable
Set the following ENVs on blockscout instance:
- `MICROSERVICE_SC_VERIFIER_ENABLED=true`
- `MICROSERVICE_SC_VERIFIER_URL={service_url}`
- `MICROSERVICE_SC_VERIFIER_TYPE=sc_verifier`

## Envs
Here, we describe variables specific to this service. Variables common to all services can be found [here](../docs/common-envs.md).

[anchor]: <> (anchors.envs.start)

| Variable                                                       | Required | Description                                                             | Default value                                                                |
|----------------------------------------------------------------|----------|-------------------------------------------------------------------------|------------------------------------------------------------------------------|
| `SMART_CONTRACT_VERIFIER__SOLIDITY__ENABLED`                   |          | Enable Solidity verification endpoints                                  | `true`                                                                       |
| `SMART_CONTRACT_VERIFIER__SOLIDITY__FETCHER__LIST__LIST_URL`   |          | Url that contains a list available Solidity compilers                   | `https://binaries.soliditylang.org/linux-amd64/list.json`                    |
| `SMART_CONTRACT_VERIFIER__SOLIDITY__REFRESH_VERSIONS_SCHEDULE` |          | Cron-format schedule to update the list of available Solidity compilers | `0 0 * * * * *`                                                              |
| `SMART_CONTRACT_VERIFIER__SOLIDITY__COMPILERS_DIR`             |          | Directory where Solidity compilers will be downloaded                   | `/tmp/solidity-compilers`                                                    |
| `SMART_CONTRACT_VERIFIER__VYPER__ENABLED`                      |          | Enable Vyper verification endpoints                                     | `true`                                                                       |
| `SMART_CONTRACT_VERIFIER__VYPER__FETCHER__LIST__LIST_URL`      |          | Url that contains a list of available Vyper compilers                   | `https://raw.githubusercontent.com/blockscout/solc-bin/main/vyper.list.json` |
| `SMART_CONTRACT_VERIFIER__VYPER__REFRESH_VERSIONS_SCHEDULE`    |          | Cron-format schedule to update the list of available Vyper compilers    | `0 0 * * * * *`                                                              |
| `SMART_CONTRACT_VERIFIER__VYPER__COMPILERS_DIR`                |          | Directory where Vyper compilers will be downloaded                      | `/tmp/vyper-compilers`                                                       |
| `SMART_CONTRACT_VERIFIER__SOURCIFY__ENABLED`                   |          | Enable Soucify verification endpoint                                    | `true`                                                                       |
| `SMART_CONTRACT_VERIFIER__SOURCIFY__API_URL`                   |          | Sourcify API url                                                        | `https://sourcify.dev/server/`                                               |
| `SMART_CONTRACT_VERIFIER__SOURCIFY__VERIFICATION_ATTEMPTS`     |          | Number of attempts the server makes to Sourcify API. Must be at least 1 | `3`                                                                          |
| `SMART_CONTRACT_VERIFIER__SOURCIFY__REQUEST_TIMEOUT`           |          | Timeout in seconds for a single request to Sourcify API                 | `15`                                                                         |
| `SMART_CONTRACT_VERIFIER__SOURCIFY__POLL_INTERVAL_MS`          |          | Interval in milliseconds between polls of an async Sourcify (v2) verification job | `1000`                                                             |
| `SMART_CONTRACT_VERIFIER__SOURCIFY__MAX_POLL_ATTEMPTS`         |          | Max number of polls of an async Sourcify (v2) verification job before giving up. Must be at least 1 | `120`                                            |
| `SMART_CONTRACT_VERIFIER__COMPILERS__MAX_THREADS`              |          | Maximum number of concurrent compilations                               | `8`                                                                          |
| `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__TYPE`          | yes      | Compiler execution mode: `docker` in production, `native` for local development | `disabled`                                                            |
| `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__EXECUTION_TIMEOUT_SECONDS` | native/docker | Maximum duration of one compiler invocation; tune with representative workloads | `600`                                                          |
| `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__MAX_OUTPUT_BYTES` | native/docker | Maximum combined compiler stdout and stderr size                       | `268435456`                                                                  |
| `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__ADDR`          | docker   | Dedicated Docker host reached through SSH                               |                                                                              |
| `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__KEY_PATH`      |          | SSH private key path inside the verifier container                      | SSH client default                                                           |
| `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__RUNNER_IMAGE`  | docker   | Runner image pinned by `@sha256:<64 hex characters>`; preloaded on the host unless `PULL_RUNNER_IMAGE` is set |                                          |
| `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__PULL_RUNNER_IMAGE` |      | Pull `RUNNER_IMAGE` onto the Docker host on first use when it is missing. Requires registry egress from that host and anonymous pull access | `false`      |
| `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__PULL_TIMEOUT_SECONDS` |    | Budget for that pull, separate from `API_TIMEOUT_SECONDS`                | `900`                                                                        |
| `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__PLATFORM`      |          | Runner platform; compiler download lists must match                     | `linux/amd64`                                                                |
| `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__CONNECT_TIMEOUT_SECONDS` | docker | Startup SSH host-key preflight timeout in seconds                       | `30`                                                                         |
| `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__API_TIMEOUT_SECONDS` | docker | Docker API request timeout in seconds, raised to at least the execution timeout; readiness checks use it alone | `30`                                                                  |
| `SMART_CONTRACT_VERIFIER__COMPILERS__EXECUTION__RUNTIME`       |          | Optional hardened runtime installed on the external Docker host         | Docker default                                                               |

[anchor]: <> (anchors.envs.end)

## Links
- Demo - https://http.sc-verifier.services.blockscout.com
- [Swagger](https://blockscout.github.io/swaggers/services/smart-contract-verifier/index.html)
- [Packages](https://github.com/blockscout/blockscout-rs/pkgs/container/smart-contract-verifier)
- [Releases](https://github.com/blockscout/blockscout-rs/releases?q=smart-contract-verifier&expanded=true)
