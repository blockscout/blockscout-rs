use httpmock::prelude::*;
use pretty_assertions::assert_eq;
use sig_provider::{sig_provider, Settings};
use std::sync::atomic::{AtomicUsize, Ordering};

static PORT: AtomicUsize = AtomicUsize::new(11000);

async fn server(fourbyte: url::Url, sigeth: url::Url) -> usize {
    let port = PORT.fetch_add(1, Ordering::SeqCst);

    let mut settings = Settings::default();
    settings.server.grpc.enabled = false;
    settings.server.http.addr = format!("[::]:{}", port).parse().unwrap();
    settings.sources.fourbyte = fourbyte;
    settings.sources.sigeth = sigeth;
    tokio::spawn(async move {
        sig_provider(settings).await.unwrap();
    });
    // allow server to start
    tokio::task::yield_now().await;

    port
}

#[tokio::test]
async fn create() {
    let expected_request = serde_json::json!({"contract_abi":"[{\"constant\":false,\"inputs\":[],\"name\":\"f\",\"outputs\":[],\"type\":\"function\"},{\"inputs\":[],\"type\":\"constructor\"},{\"anonymous\":false,\"inputs\":[{\"name\":\"\",\"type\":\"string\",\"indexed\":true}],\"name\":\"E\",\"type\":\"event\"}]"});
    let expected_response = serde_json::json!({"num_processed":25,"num_imported":3,"num_duplicates":18,"num_ignored":4});
    let fourbyte = MockServer::start();
    let fourbyte_create = fourbyte.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/import-solidity/")
            .header("Content-type", "application/json")
            .json_body(expected_request);
        then.status(201)
            .header("Content-type", "application/json")
            .json_body(expected_response);
    });

    let expected_request = serde_json::json!({"type":"abi","data":[[{"constant":false,"inputs":[],"name":"f","outputs":[],"type":"function"},{"inputs":[],"type":"constructor"},{"anonymous":false,"inputs":[{"name":"","type":"string","indexed":true}],"name":"E","type":"event"}]]});
    let expected_response = serde_json::json!({"ok":true,"result":{"event":{"imported":{},"duplicated":{"E(string)":"0x3e9992c940c54ea252d3a34557cc3d3014281525c43d694f89d5f3dfd820b07d"},"invalid":null},"function":{"imported":{},"duplicated":{"f()":"0x26121ff0"},"invalid":null}}});
    let sigeth = MockServer::start();
    let sigeth_create = sigeth.mock(|when, then| {
        when.method(POST)
            .path("/api/v1/import")
            .header("Content-type", "application/json")
            .json_body(expected_request);
        then.status(200)
            .header("Content-type", "application/json")
            .json_body(expected_response);
    });

    let port = server(
        format!("http://127.0.0.1:{}/", fourbyte.port())
            .parse()
            .unwrap(),
        format!("http://127.0.0.1:{}/", sigeth.port())
            .parse()
            .unwrap(),
    )
    .await;

    let client = reqwest::Client::new();
    let request = serde_json::json!({"abi":"[{\"constant\":false,\"inputs\":[],\"name\":\"f\",\"outputs\":[],\"type\":\"function\"},{\"inputs\":[],\"type\":\"constructor\"},{\"anonymous\":false,\"inputs\":[{\"name\":\"\",\"type\":\"string\",\"indexed\":true}],\"name\":\"E\",\"type\":\"event\"}]"});
    let response: serde_json::Value = client
        .post(format!("http://127.0.0.1:{}/api/v1/signatures", port))
        .json(&request)
        .header("Content-type", "application/json")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(serde_json::json!({}), response);
    fourbyte_create.assert();
    sigeth_create.assert();
}
