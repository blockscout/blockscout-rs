use std::{path::PathBuf, sync::Arc};

use actix_files::NamedFile;
use actix_web::{web::get, HttpRequest, Result};

async fn serve_swagger_from(path: Arc<PathBuf>, _req: HttpRequest) -> Result<NamedFile> {
    Ok(NamedFile::open(path.as_ref())?)
}

pub fn route_swagger(
    service_config: &mut actix_web::web::ServiceConfig,
    swagger_file_path: PathBuf,
    route: &str,
) {
    let path = Arc::new(swagger_file_path);
    let serve_swagger = move |req: HttpRequest| serve_swagger_from(path.clone(), req);
    service_config.configure(|config| {
        config.route(route, get().to(serve_swagger));
    });
}
