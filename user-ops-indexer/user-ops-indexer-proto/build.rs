use actix_prost_build::{ActixGenerator, GeneratorList};
use prost_build::{Config, ServiceGenerator};
use std::path::Path;

// custom function to include custom generator
fn compile(
    protos: &[impl AsRef<Path>],
    includes: &[impl AsRef<Path>],
    generator: Box<dyn ServiceGenerator>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut config = Config::new();
    config
        .service_generator(generator)
        .compile_well_known_types()
        .protoc_arg("--openapiv2_out=swagger")
        .protoc_arg("--openapiv2_opt")
        .protoc_arg("grpc_api_configuration=proto/api_config_http.yaml,output_format=yaml,allow_merge=true,merge_file_name=user-ops-indexer,json_names_for_fields=false")
        .bytes(["."])
        .type_attribute(".", "#[actix_prost_macros::serde(rename_all=\"snake_case\")]")
        .field_attribute(
            ".blockscout.userOpsIndexer.v1.HealthCheckRequest.service",
            "#[serde(default)]"
        )
        .extern_path(".google.protobuf", "::prost-wkt-types");

    config.compile_protos(protos, includes)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // We need to rebuild proto lib only if any of proto definitions
    // (or corresponding http mapping) has been changed.
    println!("cargo:rerun-if-changed=proto/");

    std::fs::create_dir_all("./swagger").unwrap();
    let gens = Box::new(GeneratorList::new(vec![
        tonic_build::configure().service_generator(),
        Box::new(ActixGenerator::new("proto/api_config_http.yaml").unwrap()),
    ]));
    compile(
        &["proto/user-ops-indexer.proto", "proto/health.proto"],
        &["proto"],
        gens,
    )?;
    Ok(())
}
