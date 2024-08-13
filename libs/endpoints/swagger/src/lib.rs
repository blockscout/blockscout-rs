use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use actix_files::NamedFile;
use actix_web::{web::get, HttpRequest, Result};
use prost_build::Config;

async fn serve_swagger_from(path: Arc<PathBuf>, _req: HttpRequest) -> Result<NamedFile> {
    Ok(NamedFile::open(path.as_ref())?)
}

pub fn register_route(
    service_config: &mut actix_web::web::ServiceConfig,
    swagger_file_path: PathBuf,
) {
    let path = Arc::new(swagger_file_path);
    let serve_swagger = move |req: HttpRequest| serve_swagger_from(path.clone(), req);
    service_config.configure(|config| {
        config.route("/api/v1/docs/swagger.yaml", get().to(serve_swagger));
    });
}
