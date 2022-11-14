use actix_web::App;
use httpmock::MockServer;
use pretty_assertions::assert_eq;
use serde_json::Value;
use sig_provider_server::{http_configure, new_service, SourcesSettings};
use std::time::Duration;

#[tokio::test]
async fn create() {
    let _ = env_logger::builder().is_test(true).try_init();

    let fourbyte_request = serde_json::json!({"contract_abi":"[{\"constant\":false,\"inputs\":[],\"name\":\"f\",\"outputs\":[],\"type\":\"function\"},{\"inputs\":[],\"type\":\"constructor\"},{\"anonymous\":false,\"inputs\":[{\"name\":\"\",\"type\":\"string\",\"indexed\":true}],\"name\":\"E\",\"type\":\"event\"}]"});
    let fourbyte_response = serde_json::json!({"num_processed":25,"num_imported":3,"num_duplicates":18,"num_ignored":4});
    let fourbyte = MockServer::start();
    let fourbyte_handle = fourbyte.mock(|when, then| {
        when.method(httpmock::Method::POST)
            .path("/api/v1/import-solidity/")
            .header("Content-type", "application/json")
            .json_body(fourbyte_request);
        then.status(201)
            .header("Content-type", "application/json")
            .json_body(fourbyte_response);
    });

    let sigeth_request = serde_json::json!({"type":"abi","data":[[{"constant":false,"inputs":[],"name":"f","outputs":[],"type":"function"},{"inputs":[],"type":"constructor"},{"anonymous":false,"inputs":[{"name":"","type":"string","indexed":true}],"name":"E","type":"event"}]]});
    let sigeth_response = serde_json::json!({"ok":true,"result":{"event":{"imported":{},"duplicated":{"E(string)":"0x3e9992c940c54ea252d3a34557cc3d3014281525c43d694f89d5f3dfd820b07d"},"invalid":null},"function":{"imported":{},"duplicated":{"f()":"0x26121ff0"},"invalid":null}}});
    let sigeth = MockServer::start();
    let sigeth_handle = sigeth.mock(|when, then| {
        when.method(httpmock::Method::POST)
            .path("/api/v1/import")
            .header("Content-type", "application/json")
            .json_body(sigeth_request);
        then.status(200)
            .header("Content-type", "application/json")
            .json_body(sigeth_response);
    });

    let service = new_service(SourcesSettings {
        fourbyte: format!("http://127.0.0.1:{}/", fourbyte.port())
            .parse()
            .unwrap(),
        sigeth: format!("http://127.0.0.1:{}/", sigeth.port())
            .parse()
            .unwrap(),
    });
    let app = actix_web::test::init_service(
        App::new().configure(|config| http_configure(config, service.clone(), service.clone())),
    )
    .await;

    let request = serde_json::json!({"abi":"[{\"constant\":false,\"inputs\":[],\"name\":\"f\",\"outputs\":[],\"type\":\"function\"},{\"inputs\":[],\"type\":\"constructor\"},{\"anonymous\":false,\"inputs\":[{\"name\":\"\",\"type\":\"string\",\"indexed\":true}],\"name\":\"E\",\"type\":\"event\"}]"});
    let request = actix_web::test::TestRequest::default()
        .method(http::Method::POST)
        .uri("/api/v1/signatures")
        .append_header(("Content-type", "application/json"))
        .set_json(request)
        .to_request();
    let response: serde_json::Value = actix_web::test::call_and_read_body_json(&app, request).await;
    // allow async handle to work
    tokio::time::sleep(Duration::from_millis(100)).await;

    fourbyte_handle.assert();
    sigeth_handle.assert();
    assert_eq!(serde_json::json!({}), response);
}

fn sort_json_mut(v: &mut Value) {
    match v {
        Value::Array(arr) => {
            arr.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
        }
        Value::Object(obj) => {
            for (_, val) in obj.iter_mut() {
                sort_json_mut(val);
            }
        }
        _ => (),
    }
}

fn sort_json(mut v: Value) -> Value {
    sort_json_mut(&mut v);
    v
}

#[tokio::test]
async fn get_function() {
    let _ = env_logger::builder().is_test(true).try_init();

    let fourbyte_response = serde_json::json!({"count":4,"next":null,"previous":null,"results":[{"id":844293,"created_at":"2022-08-26T12:22:13.363345Z","text_signature":"watch_tg_invmru_119a5a98(address,uint256,uint256)","hex_signature":"0x70a08231","bytes_signature":"p 1"},{"id":166551,"created_at":"2019-09-24T11:36:57.296021Z","text_signature":"passphrase_calculate_transfer(uint64,address)","hex_signature":"0x70a08231","bytes_signature":"p 1"},{"id":166550,"created_at":"2019-09-24T11:36:37.525020Z","text_signature":"branch_passphrase_public(uint256,bytes8)","hex_signature":"0x70a08231","bytes_signature":"p 1"},{"id":143,"created_at":"2016-07-09T03:58:27.545013Z","text_signature":"balanceOf(address)","hex_signature":"0x70a08231","bytes_signature":"p 1"}]});
    let fourbyte = MockServer::start();
    let fourbyte_handle = fourbyte.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/api/v1/signatures/")
            .query_param("hex_signature", "70a08231");
        then.status(200)
            .header("Content-type", "application/json")
            .json_body(fourbyte_response);
    });

    let sigeth_response = serde_json::json!({"ok":true,"result":{"event":{},"function":{"0x70a08231":[{"name":"passphrase_calculate_transfer(uint64,address)","filtered":true},{"name":"branch_passphrase_public(uint256,bytes8)","filtered":true},{"name":"balanceOf(address)","filtered":false}]}}});
    let sigeth = MockServer::start();
    let sigeth_handle = sigeth.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/api/v1/signatures")
            .query_param("function", "0x70a08231")
            .query_param_exists("all");
        then.status(200)
            .header("Content-type", "application/json")
            .json_body(sigeth_response);
    });

    let service = new_service(SourcesSettings {
        fourbyte: format!("http://127.0.0.1:{}/", fourbyte.port())
            .parse()
            .unwrap(),
        sigeth: format!("http://127.0.0.1:{}/", sigeth.port())
            .parse()
            .unwrap(),
    });
    let app = actix_web::test::init_service(
        App::new().configure(|config| http_configure(config, service.clone(), service.clone())),
    )
    .await;

    let request = actix_web::test::TestRequest::default()
        .method(http::Method::GET)
        .uri("/api/v1/abi/function?txInput=0x70a0823100000000000000000000000000000000219ab540356cbb839cbe05303d7705fa")
        .to_request();
    let response: serde_json::Value = actix_web::test::call_and_read_body_json(&app, request).await;

    fourbyte_handle.assert();
    sigeth_handle.assert();

    assert_eq!(
        sort_json(
            serde_json::json!([{"inputs":[{"components":[],"indexed":null,"name":"arg0","type":"address","value":"00000000219ab540356cbb839cbe05303d7705fa"}],"name":"balanceOf"}])
        ),
        sort_json(response)
    );
}

#[tokio::test]
async fn get_event() {
    let _ = env_logger::builder().is_test(true).try_init();

    let fourbyte_response = serde_json::json!({"count":1,"next":null,"previous":null,"results":[{"id":1,"created_at":"2020-11-30T22:38:00.801049Z","text_signature":"Transfer(address,address,uint256)","hex_signature":"0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef","bytes_signature":"ÝòR­\u{001}bâÈiÂ°hü7ª+§ñcÄ¡\u{0016}(õZMõ#³ï"}]});
    let fourbyte = MockServer::start();
    let fourbyte_handle = fourbyte.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/api/v1/event-signatures/")
            .query_param(
                "hex_signature",
                "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
            );
        then.status(200)
            .header("Content-type", "application/json")
            .json_body(fourbyte_response);
    });

    let sigeth_response = serde_json::json!({"ok":true,"result":{"event":{"0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef":[{"name":"Transfer(address,address,uint256)","filtered":false}]},"function":{}}});
    let sigeth = MockServer::start();
    let sigeth_handle = sigeth.mock(|when, then| {
        when.method(httpmock::Method::GET)
            .path("/api/v1/signatures")
            .query_param(
                "event",
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
            )
            .query_param_exists("all");
        then.status(200)
            .header("Content-type", "application/json")
            .json_body(sigeth_response);
    });

    let service = new_service(SourcesSettings {
        fourbyte: format!("http://127.0.0.1:{}/", fourbyte.port())
            .parse()
            .unwrap(),
        sigeth: format!("http://127.0.0.1:{}/", sigeth.port())
            .parse()
            .unwrap(),
    });
    let app = actix_web::test::init_service(
        App::new().configure(|config| http_configure(config, service.clone(), service.clone())),
    )
    .await;

    let request = actix_web::test::TestRequest::default()
        .method(http::Method::GET)
        .uri("/api/v1/abi/event?topics=0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef,000000000000000000000000b8ace4d9bc469ddc8e788e636e817c299a1a8150,000000000000000000000000f76c5b19e86c256482f4aad1dae620a0c3ac0cd6&data=00000000000000000000000000000000000000000000000000000000006acfc0")
        .to_request();
    let response: serde_json::Value = actix_web::test::call_and_read_body_json(&app, request).await;

    fourbyte_handle.assert();
    sigeth_handle.assert();

    assert_eq!(
        sort_json(
            serde_json::json!([{"inputs":[{"components":[],"indexed":true,"name":"arg0","type":"address","value":"b8ace4d9bc469ddc8e788e636e817c299a1a8150"},{"components":[],"indexed":true,"name":"arg1","type":"address","value":"f76c5b19e86c256482f4aad1dae620a0c3ac0cd6"},{"components":[],"indexed":false,"name":"arg2","type":"uint256","value":"6acfc0"}],"name":"Transfer"}])
        ),
        sort_json(response),
    );
}
