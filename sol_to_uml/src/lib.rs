pub mod config;
mod cli;

use crate::config::Config;
use actix_web::{error, web, App, HttpResponse, HttpServer, http::{header, StatusCode}};
use actix_files::NamedFile;
use std::error::Error;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use actix_multipart::Multipart;
use anyhow::private::format_err;
use derive_more::{Display, Error as DeriveError};
use futures_util::TryStreamExt as _;
use log::error;
use uuid::Uuid;

#[derive(Debug, Display, DeriveError)]
enum MyError {
    #[display(fmt = "internal error")]
    InternalError,

    #[display(fmt = "bad request")]
    BadClientData,

    #[display(fmt = "timeout")]
    Timeout,
}

impl error::ResponseError for MyError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header(header::ContentType::html())
            .body(self.to_string())
    }

    fn status_code(&self) -> StatusCode {
        match *self {
            MyError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            MyError::BadClientData => StatusCode::BAD_REQUEST,
            MyError::Timeout => StatusCode::GATEWAY_TIMEOUT,
        }
    }
}

async fn proceed_file(tmp_dir: web::Data<PathBuf>, mut payload: Multipart) -> Result<NamedFile, Box<dyn Error>> {
    // iterate over multipart stream
    while let Some(mut field) = payload.try_next().await? {
        let contract_name = Uuid::new_v4().to_string();
        let contract_path = tmp_dir.get_ref().clone().join(format!("{contract_name}.sol"));
        let contract_path_copy = contract_path.clone();

        // File::create is blocking operation, use threadpool
        let mut f = web::block(move || std::fs::File::create(contract_path_copy)).await??;

        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.try_next().await? {
            // filesystem operations are blocking, we have to use threadpool
            f = web::block(move || f.write_all(&chunk).map(|_| f)).await??;
        }

        let uml_name = Uuid::new_v4().to_string();
        let uml_path = tmp_dir.get_ref().clone().join(format!("{uml_name}.svg"));

        let status = Command::new("node")
            .arg("./sol_to_uml/src/npm/node_modules/sol2uml/lib/sol2uml.js")
            .arg(contract_path)
            .arg(format!("-o"))
            .arg(uml_path.as_path())
            .status()
            .expect("sol2uml command failed to start");

        log::info!("process finished with: {}, uml_path: {:?}", status, uml_path);

        if status.success() {
            return Ok(NamedFile::open(uml_path)?);
        } else {
            return Err(Box::new(MyError::InternalError));
        }

    }

    Err(Box::new(MyError::BadClientData))
}

async fn index() -> HttpResponse {
    let html = r#"<html>
        <head><title>Upload Test</title></head>
        <body>
            <form target="/" method="post" enctype="multipart/form-data">
                <input type="file" multiple name="file"/>
                <button type="submit">Submit</button>
            </form>
        </body>
    </html>"#;

    HttpResponse::Ok().body(html)
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
                web::resource("/")
                .route(web::get().to(index))
                .route(web::post().to(proceed_file)),
        )
    })
    .bind(socket_addr)?
    .run()
    .await
}
