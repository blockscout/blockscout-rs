# Build

## Using docker
Each service contains a Dockerfile in the service root directory.

## Building from source

### Preparation

1. Install rustup from [[rustup.rs](https://rustup.rs/)].

2. Make sure that openssl is installed:

    - macOS:

      `$ brew install openssl@1.1`

    - Arch Linux

      `$ sudo pacman -S pkg-config openssl`

    - Debian and Ubuntu

      `$ sudo apt-get install pkg-config libssl-dev`

    - Fedora

      `$ sudo dnf install pkg-config openssl-devel`

3. Make sure protobuf is installed and the version (`$ protoc --version`) is at least `v3.15.0`.

   If protobuf version is too old, you may see the following error: `Explicit 'optional' labels are disallowed in the Proto3 syntax.`

4. Install [`protoc-gen-openapiv2`](https://github.com/grpc-ecosystem/grpc-gateway#installation).
   You may find useful the following [Action](https://github.com/blockscout/blockscout-rs/blob/main/.github/actions/deps/action.yml#L21)
   we use in our Github pipeline.

   If not installed, you may see the following error: `Error: Custom { kind: Other, error: "protoc failed: Unknown flag: --openapiv2_opt\n" }`

### Build
1. Clone the repository and enter the service directory
    ```shell
    git clone git@github.com:blockscout/blockscout-rs.git
    cd blockscout-rs/{service-name}
    ```

2. Build the release version:
   ```shell
      cargo build --release --bin {service-name}-server
    ```

3. You can find the built binary in `target/release/` folder.

## Installing through cargo

Another way to install the binary without cloning the repository is to use cargo straightway:

```console
cargo install --git https://github.com/blockscout/blockscout-rs {service-name}-server
```

In that case, you can run the binary using just `{service-name}-server`.

## Justfile

Most of the services contain `justfile`s inside root directories (https://github.com/casey/just). 
Sometimes using that during development may make your a little easier. 
