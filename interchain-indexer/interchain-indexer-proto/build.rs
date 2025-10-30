#![allow(clippy::single_element_loop)]

use actix_prost_build::{ActixGenerator, GeneratorList};
use prost_build::{Config, ServiceGenerator};
use prost_wkt_build::*;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

// custom function to include custom generator
fn compile(
    protos: &[impl AsRef<Path>],
    includes: &[impl AsRef<Path>],
    generator: Box<dyn ServiceGenerator>,
) -> Result<(), Box<dyn std::error::Error>> {
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR environment variable not set"));
    let descriptor_file = out.join("file_descriptor_set.bin");
    let swagger_dir = "swagger/v1";
    let swagger_filename = "interchain-indexer";
    let mut config = Config::new();
    config
        .service_generator(generator)
        .file_descriptor_set_path(descriptor_file.clone())
        .compile_well_known_types()
        .protoc_arg(format!("--openapiv2_out={swagger_dir}"))
        .protoc_arg("--openapiv2_opt")
        .protoc_arg(format!("grpc_api_configuration=proto/v1/api_config_http.yaml,output_format=yaml,allow_merge=true,merge_file_name={swagger_filename},json_names_for_fields=false"))
        .bytes(["."])
        .btree_map(["."])
        .type_attribute(".", "#[actix_prost_macros::serde(rename_all=\"snake_case\")]")
        .retain_enum_prefix()
        .extern_path(".google.protobuf", "::prost_wkt_types")
        ;
    let default_fields: &[&str] = &[];
    for default_field in default_fields {
        config.field_attribute(
            format!(".blockscout.interchain-indexer.v1.{default_field}"),
            "#[serde(default)]",
        );
    }
    config.compile_protos(protos, includes)?;
    let descriptor_bytes = fs::read(descriptor_file).unwrap();
    let descriptor = FileDescriptorSet::decode(&descriptor_bytes[..]).unwrap();
    prost_wkt_build::add_serde(out, descriptor);
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // We need to rebuild proto lib only if any of proto definitions
    // (or corresponding http mapping) has been changed.
    println!("cargo:rerun-if-changed=proto/");

    std::fs::create_dir_all("./swagger/v1").unwrap();
    let gens = Box::new(GeneratorList::new(vec![
        tonic_build::configure().service_generator(),
        Box::new(ActixGenerator::new("proto/v1/api_config_http.yaml").unwrap()),
    ]));
    compile(
        &["proto/v1/interchain_indexer.proto", "proto/v1/health.proto"],
        &["proto", "../../proto"],
        gens,
    )?;
    Ok(())
}
