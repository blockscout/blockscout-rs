use std::{collections::HashMap, error::Error, net::TcpListener, str, time};

use actix_web::{
    dev::{RequestHead, Server},
    web,
    web::{Bytes, Data, Json},
    App, HttpRequest, HttpServer, Responder,
};
use awc::{Client, ClientRequest};
use futures::{stream, StreamExt};
use serde_json::Value;
use url::Url;

pub use crate::settings::{BlockscoutSettings, Instance, Settings};

pub mod settings;

#[derive(Clone, Debug)]
pub struct ApiEndpoints {
    apis: Vec<(Instance, Url)>,
    concurrent_requests: usize,
    request_timeout: time::Duration,
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
            request_timeout: settings.request_timeout,
        }
    }
}

impl ApiEndpoints {
    async fn make_request(request: ClientRequest, body: Bytes) -> Result<String, Box<dyn Error>> {
        let mut response = request.send_body(body.clone()).await?;
        let bytes = response.body().await?;
        let str = str::from_utf8(bytes.as_ref())?.to_string();
        Ok(str)
    }

    async fn make_requests(
        self,
        query: &str,
        body: Bytes,
        request_head: &RequestHead,
    ) -> Vec<(Instance, String)> {
        let client = Client::builder().timeout(self.request_timeout).finish();

        stream::iter(self.apis)
            .map(|(instance, mut url)| {
                url.set_query(Some(query));
                (instance, url.to_string())
            })
            .map(|(instance, url)| async {
                let request = client.request_from(url, request_head);
                (
                    instance,
                    ApiEndpoints::make_request(request, body.clone())
                        .await
                        .unwrap_or_else(|e| e.to_string()),
                )
            })
            .buffer_unordered(self.concurrent_requests)
            .collect()
            .await
    }
}

type Responses = HashMap<String, HashMap<String, Value>>;

fn merge_responses(json_responses: Vec<(Instance, String)>) -> Responses {
    let mut result: Responses = HashMap::new();

    json_responses
        .into_iter()
        .for_each(|(Instance(net, subnet), value)| {
            let kv_subnet = result.entry(net).or_insert_with(HashMap::new);
            kv_subnet.insert(
                subnet,
                serde_json::from_str(&value)
                    .unwrap_or_else(|e| Value::String(format!("{}: {}\n", e, value))),
            );
        });

    result
}

pub async fn handle_request(
    request: HttpRequest,
    apis_endpoints: Data<ApiEndpoints>,
    body: Bytes,
) -> impl Responder {
    let responses = apis_endpoints
        .get_ref()
        .clone()
        .make_requests(request.query_string(), body, request.head())
        .await;
    Json(merge_responses(responses))
}

pub fn run(settings: Settings) -> Result<Server, std::io::Error> {
    let listener = TcpListener::bind(settings.server.addr)?;

    let apis_endpoints: Data<ApiEndpoints> = Data::new(settings.blockscout.try_into().unwrap());

    let server = HttpServer::new(move || {
        App::new()
            .app_data(apis_endpoints.clone())
            .default_service(web::route().to(handle_request))
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
