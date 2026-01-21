use actix_prost_build::{ActixGenerator, GeneratorList};
use prost_build::{Config, ServiceGenerator};
use prost_wkt_build::{FileDescriptorSet, Message};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn prepare_comma_separated_fields(
    config: &mut Config,
    fields: impl IntoIterator<Item = (&'static str, &'static str)>,
) -> &mut Config {
    for (message, field) in fields {
        config.type_attribute(message, "#[serde_with::serde_as]")
            .field_attribute(format!("{}.{}", message, field), "#[serde_as(as = \"serde_with::StringWithSeparator::<serde_with::formats::CommaSeparator, String>\")]")
            .field_attribute(format!("{}.{}", message, field), "#[serde(default)]");
    }
    config
}

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
        .field_attribute("TokenType.TOKEN_TYPE_ERC_20", "#[serde(rename = \"ERC-20\")]")
        .field_attribute("TokenType.TOKEN_TYPE_ERC_721", "#[serde(rename = \"ERC-721\")]")
        .field_attribute("TokenType.TOKEN_TYPE_ERC_1155", "#[serde(rename = \"ERC-1155\")]")
        .field_attribute("TokenType.TOKEN_TYPE_ERC_404", "#[serde(rename = \"ERC-404\")]")
        .field_attribute("TokenType.TOKEN_TYPE_ERC_7802", "#[serde(rename = \"ERC-7802\")]")
        .field_attribute("TokenType.TOKEN_TYPE_ZRC_2", "#[serde(rename = \"ZRC-2\")]")
        // Make import fields optional
        .field_attribute("BatchImportRequest.addresses", "#[serde(default)]")
        .field_attribute("BatchImportRequest.block_ranges", "#[serde(default)]")
        .field_attribute("BatchImportRequest.hashes", "#[serde(default)]")
        .field_attribute("BatchImportRequest.interop_messages", "#[serde(default)]")
        .field_attribute("BatchImportRequest.address_coin_balances", "#[serde(default)]")
        .field_attribute("BatchImportRequest.address_token_balances", "#[serde(default)]")
        .field_attribute("BatchImportRequest.counters", "#[serde(default)]")
        .field_attribute("BatchImportRequest.tokens", "#[serde(default)]")
        .field_attribute("BatchImportRequest.AddressImport.token_type", "#[serde(default)]")
        .field_attribute("BatchImportRequest.TokenImport.Metadata.token_type", "#[serde(default)]")
        // Other Optional fields
        .field_attribute("ListAddressTokensRequest.type", "#[serde(default)]")
        .field_attribute("QuickSearchRequest.unlimited_per_chain", "#[serde(default)]")
        .field_attribute("ClusterQuickSearchRequest.unlimited_per_chain", "#[serde(default)]")
        .extern_path(".google.protobuf", "::prost-wkt-types");

    prepare_comma_separated_fields(
        &mut config,
        vec![
            ("ListAddressesRequest", "chain_id"),
            ("ListTokensRequest", "chain_id"),
            ("ListTransactionsRequest", "chain_id"),
            ("ListNftsRequest", "chain_id"),
            ("ListDappsRequest", "chain_ids"),
            ("ListDomainsRequest", "chain_id"),
            ("ListBlockNumbersRequest", "chain_id"),
            ("ListBlocksRequest", "chain_id"),
            ("ListClusterTokensRequest", "chain_id"),
            ("ListAddressTokensRequest", "chain_id"),
            ("SearchByQueryRequest", "chain_id"),
            ("ListTokenUpdatesRequest", "chain_id"),
        ],
    );

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
        tonic_prost_build::configure().service_generator(),
        Box::new(ActixGenerator::new("proto/v1/api_config_http.yaml").unwrap()),
    ]));
    compile(
        &[
            "proto/v1/multichain-aggregator.proto",
            "proto/v1/cluster-explorer.proto",
            "proto/v1/health.proto",
        ],
        &["proto"],
        gens,
    )?;
    Ok(())
}
