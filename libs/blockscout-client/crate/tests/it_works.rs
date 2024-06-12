use blockscout_client::apis::{configuration::Configuration, *};
use pretty_assertions::assert_eq;
use rstest::*;
use stubr::Stubr;

const DEFAULT_TOKEN_HASH: &str = "0xB87b96868644d99Cc70a8565BA7311482eDEBF6e";
const DEFAULT_CONTRACT_HASH: &str = "0x8FD4596d4E7788a71F82dAf4119D069a84E7d3f3";
const DEFAULT_TOKEN_INSTANCE_NUMBER: i32 = 1;
const DEFAULT_TX_HASH: &str = "0x4dd7e3f4522fcf2483ae422fd007492380051d87de6fdb17be71c7134e26857e";

#[rstest]
#[tokio::test]
async fn health(blockscout: Stubr) {
    let config = get_config_from_stubr(&blockscout);
    let health = health_api::health(&config)
        .await
        .expect("Failed to get health");
    assert_eq!(health.healthy, Some(true));
}

#[rstest]
#[tokio::test]
async fn blocks(blockscout: Stubr) {
    let config = get_config_from_stubr(&blockscout);
    let blocks = blocks_api::get_blocks(&config, None)
        .await
        .expect("Failed to get blocks");
    assert!(!blocks.items.is_empty());
}

#[rstest]
#[tokio::test]
async fn transactions(blockscout: Stubr) {
    let config = get_config_from_stubr(&blockscout);
    let transactions = transactions_api::get_txs(&config, None, None, None)
        .await
        .expect("Failed to get transactions");
    assert!(!transactions.items.is_empty());
}

#[rstest]
#[tokio::test]
async fn internal_transactions(blockscout: Stubr) {
    let config = get_config_from_stubr(&blockscout);
    let internal_transactions = transactions_api::get_internal_txs(&config, DEFAULT_TX_HASH)
        .await
        .expect("Failed to get transactions");
    assert!(!internal_transactions.items.is_empty());
}

#[rstest]
#[tokio::test]
async fn smart_contracts(blockscout: Stubr) {
    let config = get_config_from_stubr(&blockscout);
    let smart_contracts = smart_contracts_api::get_smart_contracts(&config, None, None)
        .await
        .expect("Failed to get transactions");
    assert!(!smart_contracts.items.is_empty());
    let _smart_contract = smart_contracts_api::get_smart_contract(&config, DEFAULT_CONTRACT_HASH)
        .await
        .expect("Failed to get transactions");
}

#[rstest]
#[tokio::test]
async fn tokens(blockscout: Stubr) {
    let config = get_config_from_stubr(&blockscout);
    let tokens = tokens_api::get_tokens_list(&config, None, None)
        .await
        .expect("Failed to get transactions");
    assert!(!tokens.items.is_empty());

    let _token = tokens_api::get_token(&config, DEFAULT_TOKEN_HASH)
        .await
        .expect("Failed to get transactions");
    let token_instances = tokens_api::get_nft_instances(&config, DEFAULT_TOKEN_HASH)
        .await
        .expect("Failed to get transactions");
    assert!(!token_instances.items.is_empty());

    let _token_instance =
        tokens_api::get_nft_instance(&config, DEFAULT_TOKEN_HASH, DEFAULT_TOKEN_INSTANCE_NUMBER)
            .await
            .expect("Failed to get transactions");
}

#[fixture]
fn blockscout() -> Stubr {
    Stubr::start_blocking("tests/recorded/eth_blockscout_com")
}

fn get_config_from_stubr(stubr: &Stubr) -> Configuration {
    Configuration::new(stubr.uri()).with_client_max_retry(3)
}
