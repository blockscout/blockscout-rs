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
