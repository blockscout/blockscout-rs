use crate::launcher::ServerSettings;
use reqwest::Url;
use std::{
    future::Future,
    net::{SocketAddr, TcpListener},
    str::FromStr,
    time::Duration,
};
use tokio::time::timeout;

fn get_free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

pub fn get_test_server_settings() -> (ServerSettings, Url) {
    let mut server = ServerSettings::default();
    let port = get_free_port();
    server.http.addr = SocketAddr::from_str(&format!("127.0.0.1:{port}")).unwrap();
    server.grpc.enabled = false;
    let base = Url::parse(&format!("http://{}", server.http.addr)).unwrap();
    (server, base)
}

pub async fn init_server<F, R>(run: F, base: &Url, health_check_service: &str)
where
    F: FnOnce() -> R + Send + 'static,
    R: Future<Output = ()> + Send,
{
    tokio::spawn(async move { run().await });

    let client = reqwest::Client::new();
    let health_endpoint = base.join("health").unwrap();

    let wait_health_check = async {
        loop {
            if let Ok(_response) = client
                .get(health_endpoint.clone())
                .query(&[("service", health_check_service)])
                .send()
                .await
            {
                break;
            }
        }
    };
    // Wait for the server to start
    if (timeout(Duration::from_secs(10), wait_health_check).await).is_err() {
        panic!("Server did not start in time");
    }
}

async fn send_annotated_request<
    Request: serde::Serialize,
    Response: for<'a> serde::Deserialize<'a>,
>(
    url: &Url,
    route: &str,
    request: &Request,
    annotation: Option<&str>,
) -> Response {
    let annotation = annotation.map(|v| format!("({v}) ")).unwrap_or_default();

    let response = reqwest::Client::new()
        .post(url.join(route).unwrap())
        .json(&request)
        .send()
        .await
        .unwrap_or_else(|_| panic!("{annotation}Failed to send request"));

    // Assert that status code is success
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.expect("Read body as text");
        panic!("({annotation})Invalid status code (success expected). Status: {status}. Message: {message}")
    }

    response
        .json()
        .await
        .unwrap_or_else(|_| panic!("({annotation})Response deserialization failed"))
}

pub async fn send_request<Request: serde::Serialize, Response: for<'a> serde::Deserialize<'a>>(
    url: &Url,
    route: &str,
    request: &Request,
) -> Response {
    send_annotated_request(url, route, request, None).await
}
