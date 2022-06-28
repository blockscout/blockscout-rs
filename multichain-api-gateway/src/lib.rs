use std::{net::TcpListener, str};

use actix_web::{dev::Server, web, App, HttpRequest, HttpResponse, HttpServer};
use futures::{stream, StreamExt};
use reqwest::Client;
use serde_json::Value;
use url::Url;

use crate::config::{BlockScoutSettings, Instance, Settings};

pub mod config;

fn build_urls(path: &str, query: &str, settings: &BlockScoutSettings) -> Vec<(Instance, Url)> {
    settings
        .instances
        .iter()
        .map(|Instance(net, subnet)| {
            let mut url = settings.base_url.clone();
            url.set_path(&format!("{}/{}{}", net, subnet, path));
            url.set_query(Some(query));
            (Instance(net.clone(), subnet.clone()), url)
        })
        .collect::<Vec<_>>()
}

async fn make_requests(
    urls: Vec<(Instance, Url)>,
    concurrent_requests: usize,
) -> Vec<(Instance, String)> {
    let client = Client::new();

    stream::iter(urls)
        .map(|(instance, url)| async {
            let resp = client.get(url).send().await.unwrap();
            (instance, resp.bytes().await)
        })
        .buffer_unordered(concurrent_requests)
        .map(|(instance, response)| match response {
            Ok(bytes) => (
                instance,
                str::from_utf8(bytes.as_ref()).unwrap().to_string(),
            ),
            Err(e) => (instance, e.to_string()),
        })
        .collect()
        .await
}

fn merge_responses(responses: Vec<(Instance, String)>) -> serde_json::Map<String, Value> {
    let mut result: serde_json::Map<String, Value> = serde_json::Map::new();

    responses
        .into_iter()
        .map(|(instance, str)| {
            (
                instance,
                serde_json::from_str(str.as_str()).unwrap_or_else(|e| Value::String(e.to_string())),
            )
        })
        .for_each(|(Instance(net, subnet), value)| {
            let kv_subnets = result
                .entry(net)
                .or_insert(Value::from(serde_json::Map::new()))
                .as_object_mut()
                .unwrap();
            kv_subnets.insert(subnet, value);
        });

    result
}

async fn handle_default_request(
    path: &str,
    query: &str,
    settings: &BlockScoutSettings,
) -> serde_json::Map<String, Value> {
    let urls = build_urls(path, query, settings);
    let responses = make_requests(urls, settings.concurrent_requests).await;
    merge_responses(responses)
}

async fn router_get(request: HttpRequest, settings: BlockScoutSettings) -> HttpResponse {
    // TODO: parse and pass custom request to appropriate handler
    let json = handle_default_request(request.path(), request.query_string(), &settings).await;
    HttpResponse::Ok().json(json)
}

#[allow(dead_code)]
async fn router_post() -> HttpResponse {
    todo!()
}

pub fn run(settings: Settings) -> Result<Server, std::io::Error> {
    let listener = TcpListener::bind(settings.server.addr)?;

    let server = HttpServer::new(move || {
        // We want to pass the settings to the handler function, so we create clones: s1, s2
        let s1 = settings.blockscout.clone();
        App::new().route(
            "/{_}", // We want to match every GET-request regardless of URL
            web::get().to(move |request| {
                let s2 = s1.clone();
                router_get(request, s2)
            }),
        )
    })
    .listen(listener)?
    .run();
    Ok(server)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_build_urls() {
        let path = "/api";
        let query = "hello=world?foo=bar";
        let settings = BlockScoutSettings {
            base_url: Url::parse("https://blockscout.com/").unwrap(),
            instances: vec![
                Instance("eth".to_string(), "mainnet".to_string()),
                Instance("etc".to_string(), "mainnet".to_string()),
            ],
            concurrent_requests: 1,
        };

        let expected = vec![
            (
                Instance("eth".to_string(), "mainnet".to_string()),
                Url::parse("https://blockscout.com/eth/mainnet/api?hello=world?foo=bar").unwrap(),
            ),
            (
                Instance("etc".to_string(), "mainnet".to_string()),
                Url::parse("https://blockscout.com/etc/mainnet/api?hello=world?foo=bar").unwrap(),
            ),
        ];

        let actual = build_urls(path, query, &settings);

        assert_eq!(actual, expected);
    }

    #[test]
    fn check_merge_responses() {
        let responses = vec![
            (
                Instance("eth".to_string(), "mainnet".to_string()),
                "{\"hello\":\"world\"}".to_string(),
            ),
            (
                Instance("xdai".to_string(), "mainnet".to_string()),
                "{\"foo\":\"bar\"}".to_string(),
            ),
            (
                Instance("xdai".to_string(), "testnet".to_string()),
                "{\"baz\":\"qux\"}".to_string(),
            ),
        ];

        let actual = merge_responses(responses);

        let expected = serde_json::Map::from_iter(vec![
            (
                "eth".to_string(),
                Value::Object(serde_json::Map::from_iter(vec![(
                    "mainnet".to_string(),
                    Value::Object(serde_json::Map::from_iter(vec![(
                        "hello".to_string(),
                        Value::String("world".to_string()),
                    )])),
                )])),
            ),
            (
                "xdai".to_string(),
                Value::Object(serde_json::Map::from_iter(vec![
                    (
                        "mainnet".to_string(),
                        Value::Object(serde_json::Map::from_iter(vec![(
                            "foo".to_string(),
                            Value::String("bar".to_string()),
                        )])),
                    ),
                    (
                        "testnet".to_string(),
                        Value::Object(serde_json::Map::from_iter(vec![(
                            "baz".to_string(),
                            Value::String("qux".to_string()),
                        )])),
                    ),
                ])),
            ),
        ]);

        assert_eq!(actual, expected);
    }
}
