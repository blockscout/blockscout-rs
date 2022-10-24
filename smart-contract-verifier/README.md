# <h1 align="center"> Smart-contract Verifier </h1>

**Smart-contract-verifier** - service for verification of EVM contracts. Ideologically, it accepts several source files as input, and gives the verification result.

Service is splillted into 2 main parts:

+ business logic: [smart-contract-verifier](./smart-contract-verifier) - provides verification interface in abstract form
+ http-server: [smart-contract-verifier-http](./smart-contract-verifier-http/) - implements http API using business logic crate

For now, we have implemented only http API, however, the transport protocol can be any and you can use business logic crate to create your own web API.

## Start

The are several ways to run smart-contract-verifier-http:


### Using docker

```console
docker run -p 8043:8043 --env-file ./smart-contract-verifier-http/config/base.env ghcr.io/blockscout/smart-contract-verifier:latest
```

### Using docker compose

You can build the provided sources using [docker-compose](./docker-compose.yaml) file presented in that directory.

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
