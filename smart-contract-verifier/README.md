# <h1 align="center"> Smart-contract Verifier </h1>

**Smart-contract-verifier** - service for verification of EVM based contracts. Ideologically, it accepts bytecode to be verified and potential source files as input and returns whether those files and bytecode correspond to each other.

The service consists of 2 parts, a verification library and a transport layer that serves requests:

+ [smart-contract-verifier](./smart-contract-verifier) - implements actual verification logic as a library and exposes an interface to be used by the transport layer;
+ A transport layer that implements some APIs over the service ([smart-contract-verifier-http](./smart-contract-verifier-http/)).

For now, only REST API over HTTP is available as the transport layer. However, the transport protocol is not limited to our implementation, and you could implement your own APIs using the library crate.

## Build
There are several ways to run the service discussed below.

**Note:** as we have only the HTTP server, we use it for descriptive purposes; in case of a custom API implementation, you should change `smart-contract-verifier-http` to the corresponding values.


### Using docker
You can build the provided sources using [Dockerfile](./Dockerfile) or [docker-compose](./docker-compose.yaml) files.

Alternatively, you can use docker images from our [registry](https://github.com/blockscout/blockscout-rs/pkgs/container/smart-contract-verifier)

### Building from source

Install rustup from rustup.rs.

```console
git clone git@github.com:blockscout/blockscout-rs.git
cd blockscout-rs/smart-contract-verifier
cargo build --release --bin smart-contract-verifier-http
```

You can find the built binary in `target/release/` folder.

### Installing through cargo

Another way to install the binary without cloning the repository is to use cargo straightway:

```console
cargo install --git https://github.com/blockscout/blockscout-rs smart-contract-verifier-http
```

In that case, you can run the binary using just `smart-contract-verifier-http`.

## Start
For the details of how to run the service, go into corresponding
transport protocol layer description:
- [smart-contract-verifier-http](./smart-contract-verifier-http/README.md)