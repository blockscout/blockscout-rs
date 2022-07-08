pub mod config;
mod cli;
mod types;

use crate::config::Config;
use types::{SolToUmlRequest, SolToUmlResponse};
use actix_web::{error, web::{self, Json}, App, HttpServer, Error};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

async fn sol_to_uml(
    tmp_dir: web::Data<PathBuf>,
    data: Json<SolToUmlRequest>,
) -> Result<Json<SolToUmlResponse>, Error> {
    let data = data.into_inner();

    let unique_name = Uuid::new_v4().to_string();
    let contract_dir = tmp_dir.get_ref().join(&unique_name);
    let contract_dir = web::block(move ||
        std::fs::create_dir(contract_dir.clone()).map(|_| contract_dir)
    ).await.map_err(error::ErrorInternalServerError)??;

    for (name, content) in data.sources.iter() {
        let file_path = contract_dir.join(name.as_path());
        let prefix = file_path.parent();
        if let Some(prefix) = prefix {
            let prefix = prefix.to_path_buf();
            web::block(move || std::fs::create_dir_all(prefix)).await.map_err(error::ErrorBadRequest)??;
        }

        let mut f = web::block(move || std::fs::File::create(file_path)).await.map_err(error::ErrorBadRequest)??;
        let content = (*content).clone();
        web::block(move || f.write_all(&content.as_bytes())).await.map_err(error::ErrorBadRequest)??;
    }

    let uml_path = contract_dir.join(format!("{unique_name}.svg"));
    let status = Command::new("sol2uml")
        .arg(&contract_dir)
        .arg(format!("-o"))
        .arg(&uml_path)
        .status()
        .expect("sol2uml command failed to start");

    log::info!("process finished with: {}, uml_path: {:?}", status, uml_path);

    if status.success() {
        let uml_diagram = web::block(move || std::fs::read_to_string(uml_path)).await.map_err(error::ErrorBadRequest)??;
        web::block(move || std::fs::remove_dir_all(contract_dir)).await.map_err(error::ErrorBadRequest)??;
        return Ok(Json(SolToUmlResponse{ uml_diagram }));
    } else {
        Err(error::ErrorBadRequest(""))
    }
}

pub async fn run(config: Config) -> std::io::Result<()> {
    let socket_addr = config.server.addr;
    let tmp_dir: web::Data<PathBuf> = web::Data::new(config.uml_creator.tmp_dir);
    if !tmp_dir.exists() {
        std::fs::create_dir_all(tmp_dir.as_path())?;
    } else if !tmp_dir.is_dir() {
        panic!("Temporary directory isn`t a directory.")
    }

    log::info!("Sol-to-uml server is starting at {}", socket_addr);
    HttpServer::new(move || {
        App::new()
            .app_data(tmp_dir.clone())
            .service(
                web::resource("/sol2uml")
                .route(web::post().to(sol_to_uml)),
        )
    })
    .bind(socket_addr)?
    .run()
    .await
}
