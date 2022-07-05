use std::{collections::HashMap, net::TcpListener, str};

use actix_web::{
    dev::Server,
    http::Method,
    web,
    web::{Bytes, Data, Json},
    App, HttpRequest, HttpServer, Responder,
};
use futures::{stream, StreamExt};
use reqwest::Client;
use serde_json::Value;
use url::Url;

use crate::config::{BlockscoutSettings, Instance, Settings};

mod cli;
pub mod config;

#[derive(Clone, Debug)]
pub struct ApiEndpoints {
    apis: Vec<(Instance, Url)>,
    concurrent_requests: usize,
}

/// Assumptions:
/// 1. All api calls expected to have trailing path "/api"
///     e.g. "<base_url>/<...>/<...>/api?<query>"
/// 2. First two segments of path of api call expected to be (network, chain)
///     e.g. "<base_url>/<network>/<chain>/<...>/<...>"
/// Taking it to account, we expect the following api call urls:
///     e.g. <base_url>/<network>/<chain>/api?<query>   
impl From<BlockscoutSettings> for ApiEndpoints {
    fn from(settings: BlockscoutSettings) -> Self {
        let mut apis = Vec::new();
        for Instance(net, subnet) in settings.instances {
            let mut url = settings.base_url.clone();
            url.set_path(&format!("{}/{}/api", net, subnet));
            apis.push((Instance(net, subnet), url));
        }
        Self {
            apis,
            concurrent_requests: settings.concurrent_requests,
        }
    }
}

impl ApiEndpoints {
    async fn make_requests(
        self,
        query: &str,
        method: &Method,
        body: Bytes,
    ) -> Vec<(Instance, String)> {
        let client = Client::new();

        stream::iter(self.apis)
            .map(|(instance, mut url)| async {
                url.set_query(Some(query));

                let resp = client
                    .request(method.clone(), url)
                    .header("Content-Type", "application/json")
                    .body(body.clone())
                    .send()
                    .await
                    .unwrap();
                (instance, resp.bytes().await)
            })
            .buffer_unordered(self.concurrent_requests)
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
}

type Responses = HashMap<String, HashMap<String, Value>>;

fn merge_responses(json_resonses: Vec<(Instance, String)>) -> Responses {
    let mut result: Responses = HashMap::new();

    json_resonses
        .into_iter()
        .for_each(|(Instance(net, subnet), value)| {
            let kv_subnet = result.entry(net).or_insert_with(HashMap::new);
            kv_subnet.insert(
                subnet,
                serde_json::from_str(&value).unwrap_or_else(|e| Value::String(e.to_string())),
            );
        });

    result
}

pub async fn router_get(
    request: HttpRequest,
    apis_endpoints: Data<ApiEndpoints>,
    body: Bytes,
) -> impl Responder {
    let responses = apis_endpoints
        .get_ref()
        .clone()
        .make_requests(request.query_string(), request.method(), body)
        .await;
    Json(merge_responses(responses))
}

pub fn run(settings: Settings) -> Result<Server, std::io::Error> {
    let listener = TcpListener::bind(settings.server.addr)?;

    let apis_endpoints: Data<ApiEndpoints> = Data::new(settings.blockscout.try_into().unwrap());

    let server = HttpServer::new(move || {
        App::new()
            .app_data(apis_endpoints.clone())
            .default_service(web::route().to(router_get))
    })
    .listen(listener)?
    .run();
    Ok(server)
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let expected = HashMap::from_iter(vec![
            (
                "eth".to_string(),
                HashMap::from_iter(vec![(
                    "mainnet".to_string(),
                    Value::Object(serde_json::Map::from_iter(vec![(
                        "hello".to_string(),
                        Value::String("world".to_string()),
                    )])),
                )]),
            ),
            (
                "xdai".to_string(),
                HashMap::from_iter(vec![
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
                ]),
            ),
        ]);

        assert_eq!(actual, expected);
    }
}
