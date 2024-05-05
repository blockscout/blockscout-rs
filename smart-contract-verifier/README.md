# <h1 align="center"> Smart-contract Verifier </h1>

**Smart-contract verifier** - service for verification of EVM based contracts. 
Its basic idea is to accept bytecode to be verified and potential source files as input and return whether those files and bytecode correspond to each other.

Is a backbone service for everything related to smart-contract verification in blockscout.
Is required if you want to have smart-contract verification functionality on your instance.

## Requirements
No additional dependencies

## How to enable
Set the following ENVs on blockscout instance:
- `MICROSERVICE_SC_VERIFIER_ENABLED=true`
- `MICROSERVICE_SC_VERIFIER_URL={service_url}`
- `MICROSERVICE_SC_VERIFIER_TYPE=sc_verifier`

## Envs
Here we describe only service specific variables. Variables common for all services can be found [here](../docs/common-envs.md).

[anchor]: <> (anchors.envs.start)

| Variable                                                       | Is required | Example value                                                                | Comment                                                                 |
|----------------------------------------------------------------| --- |------------------------------------------------------------------------------|-------------------------------------------------------------------------|
| `SMART_CONTRACT_VERIFIER__SOLIDITY__ENABLED`                   | | `true`                                                                       | Enable Solidity verification endpoints                                  |
| `SMART_CONTRACT_VERIFIER__SOLIDITY__FETCHER__LIST__LIST_URL`   | | `https://solc-bin.ethereum.org/linux-amd64/list.json`                        | Url that contains a list available Solidity compilers                   |
| `SMART_CONTRACT_VERIFIER__SOLIDITY__REFRESH_VERSIONS_SCHEDULE` | | `0 0 * * * * *`                                                              | Cron-format schedule to update the list of available Solidity compilers |
| `SMART_CONTRACT_VERIFIER__SOLIDITY__COMPILERS_DIR`             | | `/tmp/solidity-compilers`                                                    | Directory where Solidity compilers will be downloaded                   |
| `SMART_CONTRACT_VERIFIER__VYPER__ENABLED`                      | | `true`                                                                       | Enable Vyper verification endpoints                                     |
| `SMART_CONTRACT_VERIFIER__VYPER__FETCHER__LIST__LIST_URL`      | | `https://raw.githubusercontent.com/blockscout/solc-bin/main/vyper.list.json` | Url that contains a list of available Vyper compilers                   |
| `SMART_CONTRACT_VERIFIER__VYPER__REFRESH_VERSIONS_SCHEDULE`    | | `0 0 * * * * *`                                                              | Cron-format schedule to update the list of available Vyper compilers    |
| `SMART_CONTRACT_VERIFIER__VYPER__COMPILERS_DIR`                | | `/tmp/vyper-compilers`                                                       | Directory where Vyper compilers will be downloaded                      |
| `SMART_CONTRACT_VERIFIER__SOURCIFY__ENABLED`                   | | `true`                                                                       | Enable Soucify verification endpoint                                    |
| `SMART_CONTRACT_VERIFIER__SOURCIFY__API_URL`                   | | `https://sourcify.dev/server/`                                               | Sourcify API url                                                        |
| `SMART_CONTRACT_VERIFIER__SOURCIFY__VERIFICATION_ATTEMPTS`     | | `3`                                                                          | Number of attempts the server makes to Sourcify API. Must be at least 1 |
| `SMART_CONTRACT_VERIFIER__SOURCIFY__REQUEST_TIMEOUT`           | | `15`                                                                         | Timeout in seconds for a single request to Sourcify API                 |
| `SMART_CONTRACT_VERIFIER__COMPILERS__MAX_THREADS`              | | `8`                                                                          | Maximum number of concurrent compilations                               |

[anchor]: <> (anchors.envs.end)

## Links
- Demo - https://http.sc-verifier.services.blockscout.com
- [Packages](https://github.com/blockscout/blockscout-rs/pkgs/container/smart-contract-verifier)
- [Releases](https://github.com/blockscout/blockscout-rs/releases?q=smart-contract-verifier&expanded=true)