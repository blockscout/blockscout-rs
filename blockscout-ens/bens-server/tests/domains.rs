use bens_logic::test_utils::*;
use bens_server::{NetworkConfig, Settings};
use blockscout_service_launcher::{
    launcher::ConfigSettings,
    test_server::{get_test_server_settings, init_server, send_get_request, send_post_request},
};
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use sqlx::PgPool;
use url::Url;

#[sqlx::test(migrations = "../bens-logic/tests/migrations")]
async fn basic_domain_extracting_works(pool: PgPool) {
    let postgres_url = std::env::var("DATABASE_URL").expect("env should be here from sqlx::test");
    let db_url = format!(
        "{postgres_url}{}",
        pool.connect_options().get_database().unwrap()
    );
    std::env::set_var("BENS__DATABASE__CONNECT__URL", db_url);
    let clients = mocked_blockscout_clients().await;
    std::env::set_var("BENS__CONFIG", "./tests/config.test.toml");
    let mut settings = Settings::build().expect("Failed to build settings");
    let (server_settings, base) = get_test_server_settings();
    settings.server = server_settings;
    settings.blockscout.networks = clients
        .into_iter()
        .map(|(id, client)| {
            (
                id,
                NetworkConfig {
                    url: client.url().clone(),
                },
            )
        })
        .collect();

    init_server(
        || async {
            bens_server::run(settings).await.unwrap();
            Ok(())
        },
        &base,
    )
    .await;
    // Sleep until server will start and calculate all values
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // get detailed domain
    let request: Value = send_get_request(&base, "/api/v1/1/domains/vitalik.eth").await;
    assert_eq!(
        request,
        json!({
            "expiryDate": "2032-08-01T21:50:24.000Z",
            "id": "0xee6c4522aab0003e8d14cd40a6af439055fd2577951148c14b6cea9a53475835",
            "name": "vitalik.eth",
            "otherAddresses": {
                "137": "f0d485009714ce586358e3761754929904d76b9d",
                "60": "d8da6bf26964af9d7eed9e03e53415d37aa96045",
            },
            "owner": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            },
            "registrant": {
                "hash": "0x220866b1a2219f40e72f5c628b65d54268ca3a9d",
            },
            "registrationDate": "2017-06-18T08:39:14.000Z",
            "resolvedAddress": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            },
            "tokenId": "0xaf2caa1c2ca1d027f1ac823b529d0a67cd144264b2789fa2ea4d63a67c7103cc",
        })
    );

    // get events
    let expected_events = json!([
        {
            "action": "finalizeAuction",
            "fromAddress": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            },
            "timestamp": "2017-06-18T08:39:14.000000Z",
            "transactionHash": "0xdd16deb1ea750037c3ed1cae5ca20ff9db0e664a5146e5a030137d277a9247f3",
        },
        {
            "action": "transferRegistrars",
            "fromAddress": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            },
            "timestamp": "2019-07-10T05:58:51.000000Z",
            "transactionHash": "0xea30bda97a7e9afcca208d5a648e8ec1e98b245a8884bf589dec8f4aa332fb14",
        },
        {
            "action": "setAddr",
            "fromAddress": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            },
            "timestamp": "2019-10-29T13:47:34.000000Z",
            "transactionHash": "0x09922ac0caf1efcc8f68ce004f382b46732258870154d8805707a1d4b098dfd0",
        },
        {
            "action": "migrateAll",
            "fromAddress": {
                "hash": "0x0904dac3347ea47d208f3fd67402d039a3b99859",
            },
            "timestamp": "2020-02-06T18:23:40.000000Z",
            "transactionHash": "0xc3f86218c67bee8256b74b9b65d746a40bb5318a8b57948b804dbbbc3d0d7864",
        },
        {
            "action": "multicall",
            "fromAddress": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            },
            "timestamp": "2021-02-15T17:19:09.000000Z",
            "transactionHash": "0x160ef4492c731ac6b59beebe1e234890cd55d4c556f8847624a0b47125fe4f84",
        },
        {
            "action": "setResolver",
            "fromAddress": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            },
            "timestamp": "2021-02-15T17:19:17.000000Z",
            "transactionHash": "0xbb13efab7f1f798f63814a4d184e903e050b38c38aa407f9294079ee7b3110c9",
        },
    ]);
    let expected_events = expected_events.as_array().unwrap().clone();
    expect_list_results(
        &base,
        "/api/v1/1/domains/vitalik.eth/events",
        "/api/v1/1/domains/vitalik.eth/events?order=DESC",
        expected_events.clone(),
    )
    .await;
    expect_list_results(
        &base,
        "/api/v1/1/domains/vitalik.eth/events?sort=timestamp",
        "/api/v1/1/domains/vitalik.eth/events?sort=timestamp&order=DESC",
        expected_events.clone(),
    )
    .await;

    // domain lookup
    let expected_domains = vec![json!(
        {
            "id": "0x68b620f61c87062cf680144f898582a631c90e39dd1badb35c241be0a7284fff",
            "name": "sashaxyz.eth",
            "resolvedAddress": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
            },
            "owner": {
                "hash": "0x66a6f7744ce4dea450910b81a7168588f992eafb"
            },
            "registrationDate": "2021-12-24T10:23:57.000Z",
            "expiryDate": "2024-03-23T22:02:21.000Z"
        }
    )];
    expect_lookup_results(
        &base,
        "/api/v1/1/domains:lookup",
        json!({
            "name": "sashaxyz.eth"
        }),
        expected_domains.clone(),
    )
    .await;

    // address lookup
    let expected_addresses: Vec<Value> = vec![json!(
        {
            "id": "0xee6c4522aab0003e8d14cd40a6af439055fd2577951148c14b6cea9a53475835",
            "name": "vitalik.eth",
            "resolvedAddress": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
            },
            "owner": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
            },
            "registrationDate": "2017-06-18T08:39:14.000Z",
            "expiryDate": "2032-08-01T21:50:24.000Z"
        }
    )]
    .into_iter()
    .chain(expected_domains)
    .collect();
    expect_lookup_results(
        &base,
        "/api/v1/1/addresses:lookup",
        json!({
            "address": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            "resolvedTo": true,
            "ownedBy": true,
        }),
        expected_addresses.clone(),
    )
    .await;
    expect_lookup_results(
        &base,
        "/api/v1/1/addresses:lookup?sort=registration_date",
        json!({
            "address": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            "resolvedTo": true,
            "ownedBy": true,
        }),
        expected_addresses,
    )
    .await;

    // batch address resolving
    let response: Value = send_post_request(
        &base,
        "/api/v1/1/addresses:batch-resolve-names",
        &json!({
            "addresses": [
                "0xeefb13c7d42efcc655e528da6d6f7bbcf9a2251d",
                "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
                "0x9f7f7ddbfb8e14d1756580ba8037530da0880b99",
                "0x9c996076a85b46061d9a70ff81f013853a86b619",
                "0xee6c4522aab0003e8d14cd40a6af439055fd2577",
            ],
        }),
    )
    .await;
    assert_eq!(
        response,
        json!({
            "names": {
                "0x9c996076a85b46061d9a70ff81f013853a86b619": "wa🇬🇲i.eth",
                "0xd8da6bf26964af9d7eed9e03e53415d37aa96045": "vitalik.eth",
                "0xeefb13c7d42efcc655e528da6d6f7bbcf9a2251d": "test.eth",
            }
        })
    );
}

async fn expect_list_results(base: &Url, route: &str, route_desc: &str, items: Vec<Value>) {
    let request: Value = send_get_request(base, route).await;
    assert_eq!(
        request,
        json!({
            "items": items,
            "pagination": {
                "totalRecords": items.len()
            }
        })
    );

    let request: Value = send_get_request(base, route_desc).await;
    let mut reversed_items = items.clone();
    reversed_items.reverse();
    assert_eq!(
        request,
        json!({
            "items": reversed_items,
            "pagination": {
                "totalRecords": reversed_items.len()
            }
        })
    );
}

async fn expect_lookup_results(base: &Url, route: &str, mut payload: Value, items: Vec<Value>) {
    let request: Value = send_post_request(base, route, &payload).await;
    assert_eq!(
        request,
        json!({
            "items": items,
            "pagination": {
                "totalRecords": items.len()
            }
        })
    );
    payload
        .as_object_mut()
        .unwrap()
        .insert("order".to_string(), serde_json::json!("DESC"));
    let request: Value = send_post_request(base, route, &payload).await;
    let mut reversed_items = items.clone();
    reversed_items.reverse();
    assert_eq!(
        request,
        json!({
            "items": reversed_items,
            "pagination": {
                "totalRecords": reversed_items.len()
            }
        })
    );
}
