use crate::helpers;
use blockscout_service_launcher::test_server;
use pretty_assertions::assert_eq;
use proxy_verifier_proto::blockscout::proxy_verifier::v1 as proxy_verifier_v1;
use std::{
    collections::{BTreeMap, BTreeSet},
    ops::Deref,
};

#[tokio::test]
async fn test_chains_listed_in_correct_order() {
    let config_file = helpers::create_temp_config(serde_json::json!({
        "1": {
            "name": "name",
            "api_url": "https://name.blockscout.com/api/",
            "sensitive_api_key": "null"
        },
        "3": {
            "name": "name",
            "api_url": "https://name.blockscout.com/api/",
            "sensitive_api_key": "null"
        },
        "2": {
            "name": "name",
            "api_url": "https://name.blockscout.com/api/",
            "sensitive_api_key": "null"
        },
    }));
    let expected_chain_ids = ["1", "3", "2"];

    let base = helpers::init_proxy_verifier_server(|mut settings| {
        settings.chains_config = Some(config_file.as_ref().to_path_buf());
        settings
    })
    .await;

    // Check chains from `api/v1/chains` endpoint
    let response: proxy_verifier_v1::ListChainsResponse =
        test_server::send_get_request(&base, "/api/v1/chains").await;
    let actual_chain_ids: Vec<_> = response.chains.into_iter().map(|chain| chain.id).collect();
    assert_eq!(
        expected_chain_ids.as_slice(),
        actual_chain_ids.as_slice(),
        "Invalid order for `api/v1/chains` endpoint"
    );

    // Check chains from `api/v1/verification/config` endpoint
    let response: proxy_verifier_v1::VerificationConfig =
        test_server::send_get_request(&base, "/api/v1/verification/config").await;
    let actual_chain_ids: Vec<_> = response.chains.into_iter().map(|chain| chain.id).collect();
    assert_eq!(
        expected_chain_ids.as_slice(),
        actual_chain_ids.as_slice(),
        "Invalid order for `api/v1/verification/config` endpoint"
    );
}

#[tokio::test]
async fn test_returns_correct_is_testnet_field() {
    let config_file = helpers::create_temp_config(serde_json::json!({
        "1": {
            "name": "name",
            "api_url": "https://name.blockscout.com/api/",
            "sensitive_api_key": "null"
        },
        "2": {
            "name": "name",
            "api_url": "https://name.blockscout.com/api/",
            "sensitive_api_key": "null",
            "is_testnet": false
        },
        "3": {
            "name": "name",
            "api_url": "https://name.blockscout.com/api/",
            "sensitive_api_key": "null",
            "is_testnet": true
        },
    }));

    let base = helpers::init_proxy_verifier_server(|mut settings| {
        settings.chains_config = Some(config_file.as_ref().to_path_buf());
        settings
    })
    .await;

    let testnet_ids = BTreeSet::from(["3"]);

    // Check chains from `api/v1/chains` endpoint
    let response: proxy_verifier_v1::ListChainsResponse =
        test_server::send_get_request(&base, "/api/v1/chains").await;
    let chain_to_flags: BTreeMap<_, _> = response
        .chains
        .into_iter()
        .map(|chain| (chain.id, chain.is_testnet))
        .collect();
    for (chain_id, is_testnet_value) in chain_to_flags.iter() {
        assert_eq!(
            testnet_ids.contains(chain_id.deref()),
            *is_testnet_value,
            "Invalid is_testnet values for {chain_id} in `api/v1/chains` endpoint"
        );
    }

    // Check chains from `api/v1/verification/config` endpoint
    let response: proxy_verifier_v1::VerificationConfig =
        test_server::send_get_request(&base, "/api/v1/verification/config").await;
    let chain_to_flags: BTreeMap<_, _> = response
        .chains
        .into_iter()
        .map(|chain| (chain.id, chain.is_testnet))
        .collect();
    for (chain_id, is_testnet_value) in chain_to_flags.iter() {
        assert_eq!(
            testnet_ids.contains(chain_id.deref()),
            *is_testnet_value,
            "Invalid is_testnet values for {chain_id} in `api/v1/verification/config` endpoint"
        );
    }
}
