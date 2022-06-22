use std::net::TcpListener;
use std::str;

use actix_web::dev::Server;
use actix_web::web::Bytes;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer};
use futures::{stream, StreamExt};
use reqwest::Client;
use serde_json::Value;
use url::Url;

use crate::config::{BlockScoutSettings, Instance, Settings};

pub mod config;

async fn build_urls(path: &str, query: &str, settings: &BlockScoutSettings) -> Vec<Url> {
    settings
        .instances
        .iter()
        .map(|Instance(net, subnet)| {
            let mut url = settings.base_url.clone();
            url.set_path(&format!("{}/{}{}", net, subnet, path));
            url.set_query(Some(query));
            url
        })
        .collect::<Vec<_>>()
}

async fn make_get_requests(
    path: &str,
    query: &str,
    settings: &BlockScoutSettings,
) -> serde_json::Map<String, Value> {
    let client = Client::new();

    let urls = build_urls(path, query, settings).await;

    let responses: Vec<Result<Bytes, reqwest::Error>> = stream::iter(urls)
        .map(|url| {
            let client = &client;
            async move {
                let resp = client.get(url).send().await?;
                resp.bytes().await
            }
        })
        .buffered(settings.concurrent_requests)
        .collect()
        .await;

    let mut result: serde_json::Map<String, Value> = serde_json::Map::new();

    responses
        .iter()
        .map(|response| match response {
            Ok(bytes) => str::from_utf8(bytes.as_ref()).unwrap().to_string(),
            Err(e) => e.to_string(),
        })
        .map(|str| match serde_json::from_str(str.as_str()) {
            Ok(value) => value,
            Err(e) => Value::String(e.to_string()),
        })
        .zip(settings.instances.iter())
        .for_each(|(value, Instance(net, subnet))| {
            let kv_subnets = result
                .entry(net)
                .or_insert(Value::from(serde_json::Map::new()))
                .as_object_mut()
                .unwrap();
            kv_subnets.insert(subnet.to_string(), value);
        });

    result
}

async fn unification(request: HttpRequest, settings: BlockScoutSettings) -> HttpResponse {
    let s = make_get_requests(request.path(), request.query_string(), &settings).await;
    HttpResponse::Ok().json(s)
}

pub fn run(settings: Settings) -> Result<Server, std::io::Error> {
    let listener = TcpListener::bind(settings.server.addr)?;

    let server = HttpServer::new(move || {
        let s = settings.blockscout.clone();
        App::new().route(
            "/{_}",
            web::get().to(move |request| {
                let s2 = s.clone();
                unification(request, s2)
            }),
        )
    })
    .listen(listener)?
    .run();
    Ok(server)
}
