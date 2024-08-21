//! Tools for setting up `/health` endpoint.
//!
//! ## Usage
//! (example can be seen in `stats-proto` crate in this repo)
//!
//! 1. In `build.rs` in your crateuse [`add_to_compile_config`],
//!     [`proto_files`], and [`includes`] for compiling `health.proto`
//!     to rust code using [`prost_build`].
//!
//! 2. To include the generated code in your project, add the following to `lib.rs`
//!     (`/grpc.health.v1.rs` is the file name chosen based on `package` in `.proto` file):
//!     ```ignore
//!     pub mod grpc {
//!         pub mod health {
//!             pub mod v1 {
//!                 include!(concat!(env!("OUT_DIR"), "/grpc.health.v1.rs"));
//!             }
//!         }
//!     }
//!     ```
//!
//! 3. To enable swagger generation, add the following in `api_config_http.yaml`'s
//!     http rules:
//!     ```custom,{class=language-yaml}
//!         - selector: grpc.health.v1.Health.Check
//!           get: /health
//!     ```
//!
//! Now the types should be available in `grpc::health::v1` module, and swagger
//! entry for the endpoint should appear

use std::path::{Path, PathBuf};

use prost_build::Config;

pub fn add_to_compile_config(config: &mut Config) {
    config.bytes([".grpc.health"]).type_attribute(
        ".grpc.health",
        "#[actix_prost_macros::serde(rename_all=\"snake_case\")]",
    );
}

pub fn proto_files(path_to_swagger_crate: &Path) -> Vec<PathBuf> {
    vec![path_to_swagger_crate.join("proto/health.proto")]
}

pub fn includes(path_to_swagger_crate: &Path) -> Vec<PathBuf> {
    vec![path_to_swagger_crate.join("proto")]
}
