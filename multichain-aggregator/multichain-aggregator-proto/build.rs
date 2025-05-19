use actix_prost_build::{ActixGenerator, GeneratorList};
use prost_build::{Config, ServiceGenerator};
use prost_wkt_build::{FileDescriptorSet, Message};
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

    let mut config = Config::new();
    config
        .service_generator(generator)
        .file_descriptor_set_path(descriptor_file.clone())
        .compile_well_known_types()
        .protoc_arg("--openapiv2_out=swagger/v1")
        .protoc_arg("--openapiv2_opt")
        .protoc_arg("grpc_api_configuration=proto/v1/api_config_http.yaml,output_format=yaml,allow_merge=true,merge_file_name=multichain-aggregator,json_names_for_fields=false")
        .bytes(["."])
        .btree_map(["."])
        .type_attribute(".", "#[actix_prost_macros::serde(rename_all=\"snake_case\")]")
        .type_attribute(".google.protobuf", "#[derive(serde::Serialize,serde::Deserialize)]")
        // Rename token_type enum values
        .field_attribute(".blockscout.multichainAggregator.v1.BatchImportRequest.AddressImport.token_type", "#[serde(default)]")
        .field_attribute(".blockscout.multichainAggregator.v1.TokenType.TOKEN_TYPE_ERC_20", "#[serde(rename = \"ERC-20\")]")
        .field_attribute(".blockscout.multichainAggregator.v1.TokenType.TOKEN_TYPE_ERC_721", "#[serde(rename = \"ERC-721\")]")
        .field_attribute(".blockscout.multichainAggregator.v1.TokenType.TOKEN_TYPE_ERC_1155", "#[serde(rename = \"ERC-1155\")]")
        .field_attribute(".blockscout.multichainAggregator.v1.TokenType.TOKEN_TYPE_ERC_404", "#[serde(rename = \"ERC-404\")]")
        // Comma separator for ListDappsRequest.chain_ids
        .type_attribute("ListDappsRequest", "#[serde_with::serde_as]")
        .field_attribute("ListDappsRequest.chain_ids", "#[serde_as(as = \"serde_with::StringWithSeparator::<serde_with::formats::CommaSeparator, String>\")]")
        .field_attribute("ListDappsRequest.chain_ids", "#[serde(default)]")
        // Comma separator for ListTokensRequest.chain_id
        .type_attribute("ListTokensRequest", "#[serde_with::serde_as]")
        .field_attribute("ListTokensRequest.chain_id", "#[serde_as(as = \"serde_with::StringWithSeparator::<serde_with::formats::CommaSeparator, String>\")]")
        .field_attribute("ListTokensRequest.chain_id", "#[serde(default)]")
        .extern_path(".google.protobuf", "::prost-wkt-types");
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
        &[
            "proto/v1/multichain-aggregator.proto",
            "proto/v1/health.proto",
        ],
        &["proto"],
        gens,
    )?;
    Ok(())
}
