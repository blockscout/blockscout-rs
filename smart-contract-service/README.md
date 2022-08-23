# Smart-contract Service

TODO: fill info

## Build

In order to build the service you need to install the [`protoc`](https://grpc.io/docs/protoc-installation/) compiler and [`protoc-gen-openapi`](https://github.com/grpc-ecosystem/grpc-gateway)

Example install for Ubuntu:

```bash
# install protoc
apt update && apt install -y protobuf-compiler curl

# install go
LATEST_GO_VERSION="$(curl --silent https://go.dev/VERSION?m=text)";
curl -OJ -L --progress-bar https://golang.org/dl/${LATEST_GO_VERSION}.linux-amd64.tar.gz
tar -C /usr/local -xzf ${LATEST_GO_VERSION}.linux-amd64.tar.gz
export PATH=$PATH:/usr/local/go/bin

# install protoc-gen-openapi
go install github.com/grpc-ecosystem/grpc-gateway/v2/protoc-gen-openapiv2@latest
```