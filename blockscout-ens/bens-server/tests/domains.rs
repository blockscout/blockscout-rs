use bens_logic::test_utils::*;
use bens_server::Settings;
use blockscout_service_launcher::{
    launcher::ConfigSettings,
    test_server::{get_test_server_settings, init_server, send_get_request, send_post_request},
};
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use url::Url;

#[sqlx::test(migrations = "../bens-logic/tests/migrations")]
async fn basic_domain_extracting_works(pool: PgPool) {
    let postgres_url = std::env::var("DATABASE_URL").expect("env should be here from sqlx::test");
    let db_url = format!(
        "{postgres_url}{}",
        pool.connect_options().get_database().unwrap()
    );
    let blockscout_client = mocked_blockscout_client().await;
    std::env::set_var("BENS__DATABASE__CONNECT__URL", db_url);
    std::env::set_var("BENS__CONFIG", "./tests/config.test.json");
    std::env::set_var(
        "BENS__SUBGRAPHS_READER__NETWORKS__1__BLOCKSCOUT__URL",
        blockscout_client.url().to_string(),
    );
    std::env::set_var(
        "BENS__SUBGRAPHS_READER__NETWORKS__10200__BLOCKSCOUT__URL",
        blockscout_client.url().to_string(),
    );
    let mut settings = Settings::build().expect("Failed to build settings");
    let (server_settings, base) = get_test_server_settings();

    settings.server = server_settings;
    // first start with enabled cache
    check_basic_scenario_eth(settings.clone(), base.clone()).await;
    // second start with same settings to check
    // that creation of cache tables works fine
    check_basic_scenario_eth(settings.clone(), base.clone()).await;
    // third start with disabled cache
    settings.subgraphs_reader.cache_enabled = false;
    check_basic_scenario_eth(settings.clone(), base.clone()).await;
    settings.subgraphs_reader.cache_enabled = true;
    check_basic_scenario_gno(settings.clone(), base.clone()).await;
}

async fn check_basic_scenario_eth(settings: Settings, base: Url) {
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
    let vitalik_detailed_json = json!({
        "expiry_date": "2038-01-19T03:14:07.000Z",
        "id": "0xee6c4522aab0003e8d14cd40a6af439055fd2577951148c14b6cea9a53475835",
        "name": "vitalik.eth",
        "other_addresses": {
            "RSK": "0xf0d485009714cE586358E3761754929904D76B9D",
            "ETH": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045",
        },
        "owner": {
            "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
        },
        "registrant": {
            "hash": "0x220866b1a2219f40e72f5c628b65d54268ca3a9d",
        },
        "wrapped_owner": null,
        "registration_date": "2017-06-18T08:39:14.000Z",
        "resolved_address": {
            "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
        },
        "tokens": [
            {
                "id": "79233663829379634837589865448569342784712482819484549289560981379859480642508",
                "contract_hash": "0x57f1887a8bf19b14fc0df6fd9b2acc9af147ea85",
                "type": "NATIVE_DOMAIN_TOKEN",
            }
        ],
    });
    assert_eq!(request, vitalik_detailed_json.clone());
    // get detailed domain with emojied name and with wrapped token
    let request: Value = send_get_request(&base, "/api/v1/1/domains/wa🇬🇲i.eth").await;
    assert_eq!(
        request,
        json!({
            "expiry_date": "2038-01-19T03:14:07.000Z",
            "id": "0x5d438d292de31e08576d5bcd8a93aa41b401b9d9aeaba57da1a32c003e5fd5f5",
            "name": "wa🇬🇲i.eth",
            "other_addresses": {},
            "owner": {
                "hash": "0xd4416b13d2b3a9abae7acd5d6c2bbdbe25686401",
            },
            "registrant": {
                "hash": "0x9c996076a85b46061d9a70ff81f013853a86b619",
            },
            "registration_date": "2021-11-12T11:36:46.000Z",
            "resolved_address": {
                "hash": "0x9c996076a85b46061d9a70ff81f013853a86b619",
            },
            "tokens": [
                {
                    "contract_hash": "0x57f1887a8bf19b14fc0df6fd9b2acc9af147ea85",
                    "id": "46567936673033819165815925923418529171479684343878036049875289456825310839168",
                    "type": "NATIVE_DOMAIN_TOKEN",
                },
                {
                    "contract_hash": "0xd4416b13d2b3a9abae7acd5d6c2bbdbe25686401",
                    "id": "42184447928009120460686389475560276149795188091233200941948299907753855407605",
                    "type": "WRAPPED_DOMAIN_TOKEN",
                },
            ],
            "wrapped_owner": {
                "hash": "0x9c996076a85b46061d9a70ff81f013853a86b619",
            },
        })
    );

    // get events
    let expected_events = json!([
        {
            "action": "setResolver",
            "from_address": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
            },
            "timestamp": "2021-02-15T17:19:17.000000Z",
            "transaction_hash": "0xbb13efab7f1f798f63814a4d184e903e050b38c38aa407f9294079ee7b3110c9"
        },
        {
            "action": "multicall",
            "from_address": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
            },
            "timestamp": "2021-02-15T17:19:09.000000Z",
            "transaction_hash": "0x160ef4492c731ac6b59beebe1e234890cd55d4c556f8847624a0b47125fe4f84"
        },
        {
            "action": "migrateAll",
            "from_address": {
                "hash": "0x0904dac3347ea47d208f3fd67402d039a3b99859"
            },
            "timestamp": "2020-02-06T18:23:40.000000Z",
            "transaction_hash": "0xc3f86218c67bee8256b74b9b65d746a40bb5318a8b57948b804dbbbc3d0d7864"
        },
        {
            "action": "setAddr",
            "from_address": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
            },
            "timestamp": "2019-10-29T13:47:34.000000Z",
            "transaction_hash": "0x09922ac0caf1efcc8f68ce004f382b46732258870154d8805707a1d4b098dfd0"
        },
        {
            "action": "transferRegistrars",
            "from_address": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
            },
            "timestamp": "2019-07-10T05:58:51.000000Z",
            "transaction_hash": "0xea30bda97a7e9afcca208d5a648e8ec1e98b245a8884bf589dec8f4aa332fb14"
        },
        {
            "action": "finalizeAuction",
            "from_address": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
            },
            "timestamp": "2017-06-18T08:39:14.000000Z",
            "transaction_hash": "0xdd16deb1ea750037c3ed1cae5ca20ff9db0e664a5146e5a030137d277a9247f3"
        }
    ]);
    let expected_events = expected_events.as_array().unwrap().clone();
    expect_list_results(
        &base,
        "/api/v1/1/domains/vitalik.eth/events",
        Default::default(),
        expected_events.clone(),
        None,
    )
    .await;
    expect_list_results(
        &base,
        "/api/v1/1/domains/vitalik.eth/events",
        HashMap::from_iter([("sort".to_owned(), "timestamp".to_owned())]),
        expected_events.clone(),
        None,
    )
    .await;

    // all domains lookup + check pagination
    let expected_domains = [
        json!({
            "expiry_date": "2038-01-19T03:14:07.000Z",
            "id": "0x68b620f61c87062cf680144f898582a631c90e39dd1badb35c241be0a7284fff",
            "name": "sashaxyz.eth",
            "owner": {
                "hash": "0x66a6f7744ce4dea450910b81a7168588f992eafb",
            },
            "wrapped_owner": null,
            "registration_date": "2021-12-24T10:23:57.000Z",
            "resolved_address": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
            },
        }),
        json!({
            "expiry_date": "2038-01-19T03:14:07.000Z",
            "id": "0x5d438d292de31e08576d5bcd8a93aa41b401b9d9aeaba57da1a32c003e5fd5f5",
            "name": "wa🇬🇲i.eth",
            "owner": {
                "hash": "0xd4416b13d2b3a9abae7acd5d6c2bbdbe25686401",
            },
            "wrapped_owner": {
                "hash": "0x9c996076a85b46061d9a70ff81f013853a86b619",
            },
            "registration_date": "2021-11-12T11:36:46.000Z",
            "resolved_address": {
                "hash": "0x9c996076a85b46061d9a70ff81f013853a86b619",
            },
        }),
        json!({
            "expiry_date": "2038-01-19T03:14:07.000Z",
            "id": "0xeb4f647bea6caa36333c816d7b46fdcb05f9466ecacc140ea8c66faf15b3d9f1",
            "name": "test.eth",
            "owner": {
                "hash": "0xbd6bbe64bf841b81fc5a6e2b760029e316f2783b",
            },
            "wrapped_owner": null,
            "registration_date": "2019-10-24T07:26:47.000Z",
            "resolved_address": {
                "hash": "0xeefb13c7d42efcc655e528da6d6f7bbcf9a2251d",
            },
        }),
        json!({
            "expiry_date": null,
            "id": "0x6db3aa7fbaf005b22a12dd698aa41e3456ea93d2ab312796ee29fca980c99dcd",
            "name": "biglobe.eth",
            "owner": {
                "hash": "0x916a3bc6f0306426adaaa101fe28fea7a5f69b06",
            },
            "registration_date": "2017-07-08T02:11:54.000Z",
            "resolved_address": null,
            "wrapped_owner": null,
        }),
    ];
    let page_token = "1571902007".to_string();
    expect_list_results(
        &base,
        "/api/v1/1/domains:lookup",
        HashMap::from_iter([("page_size".into(), "2".into())]),
        expected_domains[0..2].to_vec(),
        Some((2, Some(page_token.clone()))),
    )
    .await;
    expect_list_results(
        &base,
        "/api/v1/1/domains:lookup",
        HashMap::from_iter([
            ("page_size".into(), "2".into()),
            ("page_token".into(), page_token.to_string()),
        ]),
        expected_domains[2..4].to_vec(),
        Some((2, Some("1499286330".into()))),
    )
    .await;

    // domain lookup
    let expected_domains = vec![json!(
        {
            "id": "0x68b620f61c87062cf680144f898582a631c90e39dd1badb35c241be0a7284fff",
            "name": "sashaxyz.eth",
            "resolved_address": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
            },
            "owner": {
                "hash": "0x66a6f7744ce4dea450910b81a7168588f992eafb"
            },
            "wrapped_owner": null,
            "registration_date": "2021-12-24T10:23:57.000Z",
            "expiry_date": "2038-01-19T03:14:07.000Z"
        }
    )];
    expect_list_results(
        &base,
        "/api/v1/1/domains:lookup",
        HashMap::from_iter([("name".into(), "sashaxyz.eth".into())]),
        expected_domains.clone(),
        Some((50, None)),
    )
    .await;

    // address lookup
    let expected_addresses: Vec<Value> = vec![json!(
        {
            "id": "0xee6c4522aab0003e8d14cd40a6af439055fd2577951148c14b6cea9a53475835",
            "name": "vitalik.eth",
            "resolved_address": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
            },
            "owner": {
                "hash": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045"
            },
            "wrapped_owner": null,
            "registration_date": "2017-06-18T08:39:14.000Z",
            "expiry_date": "2038-01-19T03:14:07.000Z"
        }
    )]
    .into_iter()
    .chain(expected_domains)
    .collect();
    expect_list_results(
        &base,
        "/api/v1/1/addresses:lookup",
        HashMap::from_iter([
            (
                "address".into(),
                "0xd8da6bf26964af9d7eed9e03e53415d37aa96045".into(),
            ),
            ("resolved_to".into(), "true".into()),
            ("owned_by".into(), "true".into()),
            ("order".into(), "ASC".into()),
            ("sort".into(), "registration_date".into()),
        ]),
        expected_addresses.clone(),
        Some((50, None)),
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
            }
        })
    );

    let response: Value = send_get_request(
        &base,
        "/api/v1/1/addresses/0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    )
    .await;
    assert_eq!(
        response,
        json!({
            "domain": vitalik_detailed_json,
            "resolved_domains_count": 2,
        })
    );
}

async fn check_basic_scenario_gno(settings: Settings, base: Url) {
    let network_id = "10200";
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

    let request: Value =
        send_get_request(&base, &format!("/api/v1/{network_id}/domains/levvv.gno")).await;
    assert_eq!(
        request,
        json!({
            "expiry_date": "2025-02-26T14:58:37.000Z",
            "id": "0xa3504cdec527495c69c760c85d5be9996252f853b91fd0df04c5b6aa2deb3347",
            "name": "levvv.gno",
            "other_addresses": {},
            "owner": {
                "hash": "0xc0de20a37e2dac848f81a93bd85fe4acdde7c0de",
            },
            "wrapped_owner": null,
            "registrant":{
                "hash": "0xc0de20a37e2dac848f81a93bd85fe4acdde7c0de",
            },
            "registration_date": "2023-11-29T09:09:25.000Z",
            "resolved_address":{
                "hash": "0xc0de20a37e2dac848f81a93bd85fe4acdde7c0de",
            },
            "tokens": [
                {
                    "id": "11990319655936053415661126359086567018700354293176496925267203544835860524390",
                    "contract_hash": "0xfd3d666db2557983f3f04d61f90e35cc696f6d60",
                    "type": "NATIVE_DOMAIN_TOKEN",
                }
            ]
        })
    );
}

async fn expect_list_results(
    base: &Url,
    route: &str,
    query_params: HashMap<String, String>,
    expected_items: Vec<Value>,
    maybe_expected_paginated: Option<(u32, Option<String>)>,
) {
    let route_with_query = build_query(route, &query_params);
    let request: Value = send_get_request(base, &route_with_query).await;
    let mut expected: HashMap<String, Value> =
        HashMap::from_iter([("items".to_owned(), json!(expected_items))]);
    if let Some((page_size, page_token)) = maybe_expected_paginated {
        if let Some(page_token) = page_token {
            expected.insert(
                "next_page_params".to_owned(),
                json!({
                    "page_token": page_token,
                    "page_size": page_size,
                }),
            );
        } else {
            expected.insert("next_page_params".to_owned(), json!(null));
        }
    }
    assert_eq!(request, json!(expected));
}

fn build_query(route: &str, query_params: &HashMap<String, String>) -> String {
    if !query_params.is_empty() {
        let query = query_params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        format!("{route}?{query}")
    } else {
        route.to_string()
    }
}
