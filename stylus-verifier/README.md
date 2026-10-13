Stylus Verifier Service
===

TODO: this is codegenerated text, change it and provide description of service

## How verification works

A project is never built on the machine the service runs on. Each verification gets a sandbox on a
dedicated Docker host, reached over SSH, and runs four steps there, each in its own non-root,
read-only container:

1. **clone** the repository at the requested commit. This step runs nothing from the repository
   and is the only one that can write to the sources. It may reach the listed hosts only.
2. **fetch** the dependencies with `cargo fetch --locked`, sources read-only, same network limits.
3. **build** with `cargo stylus get-initcode`, without any network.
4. **export the ABI** with `cargo stylus export-abi`, without any network.

The service then reads the deployment transaction from the request's `rpc_endpoint` and compares
its input with the built initcode itself. The endpoint has to resolve to a public address unless
`allow_private_rpc_endpoints` is set.

## Configuration

Settings live under `sandbox` (`STYLUS_VERIFIER__SANDBOX__...` in the environment).

| Setting | Meaning |
| --- | --- |
| `docker.addr` | `ssh://user@host` of the Docker host. Required. |
| `docker.key_path` | SSH private key. The host key must already be in `known_hosts`. |
| `docker.runner_image` | Digest-pinned runner image, present on the host. It holds the sandbox's volumes and runs its egress proxy. Required. |
| `docker.runtime` | Optional container runtime, such as `runsc`. Recommended: the steps run code from the submitted project. |
| `verifier.images` | List of `{ cargo_stylus_version, toolchain, image }`. Each image is digest-pinned, present on the host, and holds that cargo-stylus version, that Rust toolchain with the `wasm32-unknown-unknown` target and `rust-src`, and `git`. A cargo-stylus version is supported when it has at least one image; it needs `cargo stylus get-initcode`, which exists from 0.5.12. In the environment the list is a JSON string. |
| `verifier.max_concurrent_verifications` | Builds running at once; default 2. |
| `verifier.allow_private_rpc_endpoints` | Accept RPC endpoints on private addresses; default `false`. |
| `verifier.job.*` | Volume sizes, memory limits and timeouts of the steps, and `egress_hosts`. A step's memory limit also covers what it writes to its volumes. |

The remaining `docker.*` settings are those of the smart-contract-verifier's Docker executor.

## Dev

+ Install [just](https://github.com/casey/just) cli. Just is like make but better.
+ Execute `just` to see avaliable dev commands

```bash
just
```
+ Start dev postgres service by just typing

```bash
just start-postgres
```

+ Now you ready to start API server! Just run it:
```
just run
```

## Troubleshooting

1. Invalid tonic version

```
`Router` and `Router` have similar names, but are actually distinct types
```

To fix this error you need to change tonic version of `tonic` in `blockscout-service-launcer` to `0.8`

For now you can only change in `Cargo.lock`
