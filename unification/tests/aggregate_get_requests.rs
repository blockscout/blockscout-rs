use std::str;

use url::Url;

use unification::{config, run};

fn spawn_app(settings: config::Settings) {
    let server = run(settings).expect("Failed to bind address");
    let _ = tokio::spawn(server);
}

#[tokio::test]
async fn expect_result_from_two() {
    let settings = config::Settings {
        server: config::ServerSettings {
            addr: "0.0.0.0:8080".parse().unwrap(),
        },
        blockscout: config::BlockScoutSettings {
            base_url: "https://blockscout.com".parse().unwrap(),
            instances: vec![
                config::Instance("eth".to_string(), "mainnet".to_string()),
                config::Instance("xdai".to_string(), "mainnet".to_string()),
                config::Instance("xdai".to_string(), "testnet".to_string()),
            ],
            concurrent_requests: 1,
        },
    };

    spawn_app(settings.clone());

    let client = reqwest::Client::new();

    let mut url = Url::parse("http://localhost:8080/").unwrap();
    url.set_path("/api");
    url.set_query(Option::from("module=block&action=getblockreward&blockno=0"));

    let response = client
        .get(url)
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());

    let mut expected = std::fs::read_to_string("tests/res/result_from_two.json").unwrap();
    // Remove trailing newline that comes from "read_to_string"
    expected.pop();

    let bytes = response.bytes().await.unwrap();
    let str = str::from_utf8(bytes.as_ref()).unwrap().to_string();
    let actual_raw: serde_json::Value = serde_json::from_str(str.as_str()).unwrap();
    let actual = serde_json::to_string_pretty(&actual_raw).unwrap();

    assert_eq!(expected, actual);
}
