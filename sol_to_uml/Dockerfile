FROM rust:1.59 as build

# create a new empty shell project
RUN USER=root cargo new --bin sol_to_uml
WORKDIR /sol_to_uml

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./sol_to_uml/Cargo.toml ./Cargo.toml
#COPY Cargo.toml ./sol_to_uml/Cargo.toml

# this build step will cache your dependencies
RUN cargo build --release
RUN rm ./src/*.rs

# copy your source tree
COPY ./sol_to_uml/src ./src

# build for release
RUN rm ./target/release/deps/sol_to_uml*
RUN cargo build --release

# The final base image
FROM node:16-bullseye-slim

RUN apt-get update

WORKDIR /usr/src/

# sol2uml needed phantom which installation needed bzip2
RUN apt install bzip2
RUN npm install phantom
RUN npm link sol2uml --only=production

# Copy from the previous build
COPY --from=build /sol_to_uml/target/release/sol_to_uml /usr/src/sol_to_uml

# Run the binary
CMD ["/usr/src/sol_to_uml"]
