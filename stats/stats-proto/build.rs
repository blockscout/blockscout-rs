use actix_prost_build::{ActixGenerator, GeneratorList};
use prost_build::{Config, ServiceGenerator};
use std::path::{Path, PathBuf};

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
        .protoc_arg("grpc_api_configuration=proto/api_config_http.yaml,output_format=yaml,allow_merge=true,merge_file_name=stats,json_names_for_fields=false")
        .bytes(["."])
        .type_attribute(".blockscout.stats", "#[actix_prost_macros::serde(rename_all=\"snake_case\")]")
        .field_attribute(
            ".blockscout.stats.v1.HealthCheckRequest.service",
            "#[serde(default)]"
        )
        .field_attribute(".blockscout.stats.v1.Point.is_approximate", "#[serde(skip_serializing_if = \"std::ops::Not::not\")]")
        .field_attribute(".blockscout.stats.v1.Point.is_approximate", "#[serde(default)]")
        .field_attribute(".blockscout.stats.v1.GetLineChartRequest.resolution", "#[serde(default)]");
    blockscout_health_endpoint::add_to_compile_config(&mut config);

    config.compile_protos(protos, includes)?;
    Ok(())
}

fn vec_path_buf_to_string(v: Vec<PathBuf>) -> Vec<String> {
    v.into_iter()
        .map(|path| {
            path.to_str()
                .expect("Non UTF-8 paths are not supported")
                .to_string()
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let swagger_crate_folder = Path::new("../../libs/blockscout-endpoints/health");

    // We need to rebuild proto lib only if any of proto definitions
    // (or corresponding http mapping) has been changed.
    let mut proto_files_folders = ["proto/"].map(PathBuf::from).to_vec();
    proto_files_folders.extend(blockscout_health_endpoint::includes(swagger_crate_folder));
    let proto_files_folders = vec_path_buf_to_string(proto_files_folders);

    for folder in &proto_files_folders {
        println!("cargo:rerun-if-changed={folder}/");
    }

    let mut protos = ["proto/stats.proto"].map(PathBuf::from).to_vec();
    protos.extend(blockscout_health_endpoint::proto_files(
        swagger_crate_folder,
    ));
    let protos = vec_path_buf_to_string(protos);

    std::fs::create_dir_all("./swagger").unwrap();
    let gens = Box::new(GeneratorList::new(vec![
        tonic_build::configure().service_generator(),
        Box::new(ActixGenerator::new("proto/api_config_http.yaml").unwrap()),
    ]));

    compile(&protos, &proto_files_folders, gens)?;
    Ok(())
}
