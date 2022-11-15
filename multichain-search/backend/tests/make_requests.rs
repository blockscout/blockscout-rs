use actix_web::{http::StatusCode, test, web, web::Data, App};
use multichain_search::{proxy, server, Settings};
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

// TEST

#[actix_web::test]
async fn check_make_requests() {
    let mock_server = MockServer::start().await;
    let names = vec!["blockscout-1", "blockscout-2", "blockscout-3"];

    for name in names.iter() {
        Mock::given(method("GET"))
            .and(path(format!("poa/{}/api/v1/my_name", name)))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "name": name })))
            .mount(&mock_server)
            .await;
    }
    let server_host = mock_server.uri();
    let mut settings = Settings::default();
    settings.blockscout.instances = serde_json::from_value(serde_json::json!([
        {"title": "Mocked blockscout 1", "url": format!("{}/poa/blockscout-1", server_host), "id": "blockscout-1"},
        {"title": "Mocked blockscout 2", "url": format!("{}/poa/blockscout-2", server_host), "id": "blockscout-2"},
        {"title": "Mocked blockscout 3", "url": format!("{}/poa/blockscout-3", server_host), "id": "blockscout-3"},
    ])).unwrap();

    let proxy = proxy::BlockscoutProxy::new(
        settings.blockscout.instances,
        settings.blockscout.concurrent_requests,
        settings.blockscout.request_timeout,
    );

    let app = test::init_service(
        App::new()
            .app_data(Data::new(proxy.clone()))
            .default_service(web::route().to(server::handle_request)),
    )
    .await;

    let path = "/api/v1/my_name";

    let get_request = test::TestRequest::get().uri(path).to_request();
    let actual_response: proxy::Response = test::call_and_read_body_json(&app, get_request).await;
    for name in names {
        let instance_response = actual_response
            .0
            .get(name)
            .expect(&format!("response for {} not found", name));
        assert_eq!(instance_response.status, StatusCode::OK);
        assert_eq!(
            instance_response.uri.to_string(),
            format!("{}/poa/{}/api/v1/my_name", server_host, name)
        );
        assert_eq!(instance_response.instance.id, name);
        assert_eq!(
            instance_response.content,
            json!({ "name": name }).to_string()
        );
    }
}
