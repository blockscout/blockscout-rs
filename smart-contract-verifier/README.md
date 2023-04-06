# <h1 align="center"> Smart-contract Verifier </h1>

**Smart-contract-verifier** - service for verification of EVM based contracts. Ideologically, it accepts bytecode to be verified and potential source files as input and returns whether those files and bytecode correspond to each other.

The service consists of 2 parts, a verification library and a transport layer that serves requests:

+ [smart-contract-verifier](./smart-contract-verifier) - implements actual verification logic as a library and exposes an interface to be used by the transport layer;
+ A transport layer that implements some APIs over the service ([smart-contract-verifier-server](./smart-contract-verifier-server/)).

For now, GRPC and REST API over HTTP services are available as the transport layer. However, the transport protocol is not limited to our implementation, and you could implement your own APIs using the library crate.

## Build
There are several ways to run the service discussed below.

**Note:** for our description we will use an HTTP/GRPC server implementation; in case of a custom API implementation, you should change `smart-contract-verifier-server` to your values.


### Using docker
You can build the provided sources using [Dockerfile](./smart-contract-verifier-server/Dockerfile) or [docker-compose](./smart-contract-verifier-server/docker-compose.yml) files.

Alternatively, you can use docker images from our [registry](https://github.com/blockscout/blockscout-rs/pkgs/container/smart-contract-verifier)

### Building from source

Install rustup from rustup.rs.

Make sure that openssl is installed:

macOS
`$ brew install openssl@1.1`

Arch Linux
`$ sudo pacman -S pkg-config openssl`

Debian and Ubuntu
`$ sudo apt-get install pkg-config libssl-dev`

Fedora
`$ sudo dnf install pkg-config openssl-devel`

Build blockscout smart-contract-verifier:

```console
git clone git@github.com:blockscout/blockscout-rs.git
cd blockscout-rs/smart-contract-verifier
cargo build --release --bin smart-contract-verifier-server
```

You can find the built binary in `target/release/` folder.

### Installing through cargo

Another way to install the binary without cloning the repository is to use cargo straightway:

```console
cargo install --git https://github.com/blockscout/blockscout-rs smart-contract-verifier-server
```

In that case, you can run the binary using just `smart-contract-verifier-server`.

## Start
For the details of how to run the service, go into corresponding
transport protocol layer description:
- [smart-contract-verifier-server](./smart-contract-verifier-server/README.md)