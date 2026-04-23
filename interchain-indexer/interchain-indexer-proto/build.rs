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
        .field_attribute("Pagination.page_token", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("Pagination.timestamp", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("Pagination.message_id", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("Pagination.bridge_id", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("Pagination.index", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("Pagination.direction", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("BridgedTokensListPagination.page_token", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("BridgedTokensListPagination.direction", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("BridgedTokensListPagination.asset_id", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("BridgedTokensListPagination.name", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("BridgedTokensListPagination.name_blank", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("BridgedTokensListPagination.count", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("StatsChainsListPagination.page_token", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("StatsChainsListPagination.direction", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("StatsChainsListPagination.count", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("StatsChainsListPagination.chain_id", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("IndexerStatus.extra_info", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("ChainInfo.icon", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("ChainInfo.explorer", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("ChainInfo.custom_tx_route", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("ChainInfo.custom_address_route", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        .field_attribute("ChainInfo.custom_token_route", "#[serde(skip_serializing_if = \"Option::is_none\")]")
        ;
    // Enum fields that should default to 0 (first variant) when omitted from HTTP query params.
    // Cannot use `optional` in proto3 for these: that changes the generated type to Option<i32>,
    // which conflicts with actix-prost's TryFromInto<Enum> custom deserializer.
    config
        .field_attribute("GetBridgedTokensRequest.sort", "#[serde(default)]")
        .field_attribute("GetBridgedTokensRequest.order", "#[serde(default)]")
        .field_attribute("GetChainsStatsRequest.sort", "#[serde(default)]")
        .field_attribute("GetChainsStatsRequest.order", "#[serde(default)]");
    config.compile_protos(protos, includes)?;
    let descriptor_bytes = fs::read(descriptor_file).unwrap();
    let descriptor = FileDescriptorSet::decode(&descriptor_bytes[..]).unwrap();
    prost_wkt_build::add_serde(out.clone(), descriptor);
    dedupe_actix_duplicate_chain_info_internal(&out)?;
    Ok(())
}

/// actix-prost generates a fresh `ChainInfoInternal` for every message tree that nests
/// `ChainInfo`, which duplicates the struct and `TryConvert` impl in the same module.
/// Keep the first definition and drop later identical blocks (before `MessagePathRowInternal`).
fn dedupe_actix_duplicate_chain_info_internal(
    out_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = out_dir.join("blockscout.interchain_indexer.v1.rs");
    let content = fs::read_to_string(&path)?;
    let patched = strip_second_chain_info_internal_block(&content);
    if patched != content {
        fs::write(&path, patched)?;
    }
    Ok(())
}

fn strip_second_chain_info_internal_block(content: &str) -> String {
    const NEEDLE: &str = "#[derive(Clone, Debug)]\npub struct ChainInfoInternal";
    const END_BEFORE: &str = "#[derive(Clone, Debug)]\npub struct MessagePathRowInternal";
    let Some(first) = content.find(NEEDLE) else {
        return content.to_string();
    };
    let search_after = first + NEEDLE.len();
    let Some(rel_second) = content[search_after..].find(NEEDLE) else {
        return content.to_string();
    };
    let second = search_after + rel_second;
    let Some(rel_end) = content[second..].find(END_BEFORE) else {
        return content.to_string();
    };
    let mut out = String::with_capacity(content.len().saturating_sub(rel_end));
    out.push_str(&content[..second]);
    out.push_str(&content[second + rel_end..]);
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // We need to rebuild proto lib only if any of proto definitions
    // (or corresponding http mapping) has been changed.
    println!("cargo:rerun-if-changed=proto/");
    println!("cargo:rerun-if-changed=build.rs");

    std::fs::create_dir_all("./swagger/v1").unwrap();
    let gens = Box::new(GeneratorList::new(vec![
        tonic_prost_build::configure().service_generator(),
        Box::new(ActixGenerator::new("proto/v1/api_config_http.yaml").unwrap()),
    ]));
    compile(
        &[
            "proto/v1/interchain_indexer.proto",
            "proto/v1/stats.proto",
            "proto/v1/status.proto",
            "proto/v1/health.proto",
        ],
        &["proto", "../../proto"],
        gens,
    )?;
    Ok(())
}
