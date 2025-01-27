mod utils;

use crate::utils::{build_query, check_list_result, data_file_as_json, start_server};
use bens_server::Settings;
use blockscout_service_launcher::test_server::{send_get_request, send_post_request};
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::collections::HashMap;
use url::Url;

#[sqlx::test(migrations = "../bens-logic/tests/migrations")]
async fn subgraphs_reading_works(pool: PgPool) {
    let (base, settings) = start_server(&pool).await;
    get_protocols_scenario(base.clone(), &settings).await;
    eth_protocol_scenario(base.clone(), &settings).await;
    genome_protocol_scenario(base.clone(), &settings).await;
    different_protocols_scenario(base.clone(), &settings).await;
}

async fn get_protocols_scenario(base: Url, settings: &Settings) {
    let response: Value = send_get_request(&base, "/api/v1/1/protocols").await;
    let context = utils::settings_context(settings);
    assert_eq!(
        response,
        json!({
            "items": [
                data_file_as_json!("protocols/ens.json", &context),
            ]
        })
    );

    let request: Value = send_get_request(&base, "/api/v1/10200/protocols").await;
    assert_eq!(
        request,
        json!({
            "items": [
                data_file_as_json!("protocols/genome.json", &context)
            ]
        })
    );
    let request: Value = send_get_request(&base, "/api/v1/1337/protocols").await;
    assert_eq!(
        request,
        json!({
            "items": [
                data_file_as_json!("protocols/ens.json", &context),
                data_file_as_json!("protocols/genome.json", &context),
            ]
        })
    );
}

async fn eth_protocol_scenario(base: Url, settings: &Settings) {
    let context = utils::settings_context(settings);

    // get detailed domain
    let request: Value = send_get_request(&base, "/api/v1/1/domains/vitalik").await;
    let vitalik_detailed_json = data_file_as_json!("domains/vitalik_eth/detailed.json", &context);
    assert_eq!(request, vitalik_detailed_json.clone());
    // get detailed domain with emojied name and with wrapped token
    let request: Value = send_get_request(&base, "/api/v1/1/domains/wa🇬🇲i").await;
    assert_eq!(
        request,
        data_file_as_json!("domains/wai_eth/detailed.json", &context)
    );

    let request: Value = send_get_request(&base, "/api/v1/1/domains/abcnews").await;

    assert_eq!(
        request,
        data_file_as_json!("domains/abcnews_eth/detailed.json", &context)
    );

    // get events
    let expected_events = data_file_as_json!("domains/vitalik_eth/events.json", &context);
    let expected_events = expected_events.as_array().unwrap().clone();
    let (actual, expected) = check_list_result(
        &base,
        "/api/v1/1/domains/vitalik/events",
        Default::default(),
        expected_events.clone(),
        None,
    )
    .await;
    assert_eq!(actual, expected);
    let (actual, expected) = check_list_result(
        &base,
        "/api/v1/1/domains/vitalik/events",
        HashMap::from_iter([("sort".to_owned(), "timestamp".to_owned())]),
        expected_events.clone(),
        None,
    )
    .await;
    assert_eq!(actual, expected);

    // all domains lookup + check pagination
    let expected_domains = data_file_as_json!("domains/lookup_ens.json", &context)
        .as_array()
        .unwrap()
        .clone();
    let page_token = "1571902007".to_string();
    let (actual, expected) = check_list_result(
        &base,
        "/api/v1/1/domains:lookup",
        HashMap::from_iter([("page_size".into(), "2".into())]),
        expected_domains[0..2].to_vec(),
        Some((2, Some(page_token.clone()))),
    )
    .await;
    assert_eq!(actual, expected);
    let (actual, expected) = check_list_result(
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
    assert_eq!(actual, expected);

    // domain lookup
    let expected_domains = vec![data_file_as_json!(
        "domains/sashaxyz_eth/short.json",
        &context
    )];
    let (actual, expected) = check_list_result(
        &base,
        "/api/v1/1/domains:lookup",
        HashMap::from_iter([("name".into(), "sashaxyz.eth".into())]),
        expected_domains.clone(),
        Some((50, None)),
    )
    .await;
    assert_eq!(actual, expected);

    // empty domain lookup
    for invalid_name in [
        "nothing",
        "nothing.eth",
        "nothing.eth.",
        ".nothing.eth",
        ".nothing.eth.",
        ".",
        "..",
        ".........",
        " ",
        " _ ",
        " 123 ",
        " 123.eth ",
        " 1 . 2. 3. 4",
    ] {
        let request: Value = send_get_request(
            &base,
            &format!("/api/v1/1/domains:lookup?name={invalid_name}"),
        )
        .await;
        assert_eq!(
            request,
            json!({
                "items": [],
                "next_page_params": null,
            }),
            "invalid response with name: {}",
            invalid_name
        );

        let status = reqwest::get(&format!("{base}/api/v1/1/domains/{invalid_name}"))
            .await
            .unwrap()
            .status();
        assert_eq!(status, 404, "invalid status with name: {}", invalid_name);
    }

    // address lookup
    let expected_addresses: Vec<Value> = vec![json!(data_file_as_json!(
        "domains/vitalik_eth/short.json",
        &context
    ))]
    .into_iter()
    .chain(expected_domains)
    .collect();
    let (actual, expected) = check_list_result(
        &base,
        "/api/v1/1/addresses:lookup",
        HashMap::from_iter([
            ("protocols".into(), "ens".into()),
            (
                "address".into(),
                "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".into(),
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
    assert_eq!(actual, expected);

    // batch address resolving
    let response: Value = send_post_request(
        &base,
        "/api/v1/1/addresses:batch-resolve-names",
        &json!({
            "addresses": [
                "0xeefb13c7d42efcc655e528da6d6f7bbcf9a2251d",
                // unchecksummed
                "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
                "0x9f7f7ddbfb8e14d1756580ba8037530da0880b99",
                // checksummed
                "0x9C996076A85B46061D9a70ff81F013853A86b619",
                "0xee6c4522aab0003e8d14cd40a6af439055fd2577",
            ],
        }),
    )
    .await;
    assert_eq!(
        response,
        json!({
            "names": {
                "0x9C996076A85B46061D9a70ff81F013853A86b619": "wa🇬🇲i.eth",
                "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045": "vitalik.eth",
            }
        })
    );

    let response: Value = send_get_request(
        &base,
        "/api/v1/1/addresses/0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045",
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

async fn genome_protocol_scenario(base: Url, settings: &Settings) {
    let context = utils::settings_context(settings);
    let network_id = "10200";
    let request: Value =
        send_get_request(&base, &format!("/api/v1/{network_id}/domains/levvv.gno")).await;
    assert_eq!(
        request,
        data_file_as_json!("domains/levvv_gno/detailed.json", &context)
    );

    let expected_domains = data_file_as_json!("domains/lookup_genome.json", &context)
        .as_array()
        .unwrap()
        .clone();
    let page_token = "1702927825".to_string();
    let (actual, expected) = check_list_result(
        &base,
        "/api/v1/10200/domains:lookup",
        HashMap::from_iter([("page_size".into(), "2".into())]),
        expected_domains.to_vec(),
        Some((2, Some(page_token.clone()))),
    )
    .await;
    assert_eq!(actual, expected);
}

async fn different_protocols_scenario(base: Url, settings: &Settings) {
    let context = utils::settings_context(settings);
    let request: Value = send_get_request(&base, "/api/v1/1337/domains/levvv").await;
    assert_eq!(
        request,
        data_file_as_json!("domains/levvv_gno/detailed.json", &context)
    );

    let expected_domains = data_file_as_json!("domains/lookup_genome.json", &context)
        .as_array()
        .unwrap()
        .clone();
    let page_token = "1702927825".to_string();
    let (actual, expected) = check_list_result(
        &base,
        "/api/v1/1337/domains:lookup",
        HashMap::from_iter([
            ("protocols".into(), "genome".into()),
            ("page_size".into(), "2".into()),
        ]),
        expected_domains,
        Some((2, Some(page_token.clone()))),
    )
    .await;
    assert_eq!(actual, expected);

    let page_token = "1571902007".to_string();
    let expected_domains = data_file_as_json!("domains/lookup_ens.json", &context)
        .as_array()
        .unwrap()
        .clone();
    let (actual, expected) = check_list_result(
        &base,
        "/api/v1/1337/domains:lookup",
        HashMap::from_iter([
            ("protocols".into(), "ens".into()),
            ("page_size".into(), "2".into()),
        ]),
        expected_domains[0..2].to_vec(),
        Some((2, Some(page_token.clone()))),
    )
    .await;
    assert_eq!(actual, expected);

    let route_with_query = build_query(
        "/api/v1/1337/domains:lookup",
        &HashMap::from_iter([("page_size".into(), "100".into())]),
    );
    let request: Value = send_get_request(&base, &route_with_query).await;
    let names = request["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(names.contains(&"levvv.gno"));
    assert!(names.contains(&"vitalik.eth"));
}
