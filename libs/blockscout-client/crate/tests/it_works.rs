use blockscout_client::{Api, ApiClient, Configuration};
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
    let api_client = ApiClient::new(configuration(&blockscout));
    let health_api = api_client.health_api();
    let health = health_api.health().await.expect("Failed to get health");
    let success = health.try_as_success().cloned().unwrap_or_else(|| {
        panic!("failed to get health: {:?}", &health.entity);
    });
    assert_eq!(success.healthy, Some(true));
    assert!(
        success
            .metadata
            .expect("metadata is None")
            .latest_block
            .expect("latest_block is None")
            .db
            .expect("db is None")
            .number
            .expect("number is None")
            .chars().all(|c| c.is_ascii_digit()),
        
    );
    let health_v1 = health_api.health_v1().await.expect("Failed to get health");
    let success = health_v1.try_as_success().cloned().unwrap_or_else(|| {
        panic!("failed to get health: {:?}", &health_v1.entity);
    });
    assert_eq!(success.healthy, Some(true));
    assert_eq!(
        success
            .data
            .expect("data is None")
            .latest_block_number
            .expect("latest_block_number is None"),
        "21879216"
    );
}

#[rstest]
#[tokio::test]
async fn blocks(blockscout: Stubr) {
    let api_client = ApiClient::new(configuration(&blockscout));
    let blocks_api = api_client.blocks_api();
    let blocks = blocks_api
        .get_blocks(blockscout_client::apis::blocks_api::GetBlocksParams::builder().build())
        .await
        .expect("Failed to get blocks");
    let success_model = blocks.try_parse_as_success_model().unwrap();
    assert!(!success_model.items.is_empty());
}

#[rstest]
#[tokio::test]
async fn transactions(blockscout: Stubr) {
    let api_client = ApiClient::new(configuration(&blockscout));
    let transactions_api = api_client.transactions_api();
    let transactions = transactions_api
        .get_txs(blockscout_client::apis::transactions_api::GetTxsParams::builder().build())
        .await
        .expect("Failed to get transactions");
    let success_model = transactions.try_parse_as_success_model().unwrap();
    assert!(!success_model.items.is_empty());
}

#[rstest]
#[tokio::test]
async fn internal_transactions(blockscout: Stubr) {
    let api_client = ApiClient::new(configuration(&blockscout));
    let transactions_api = api_client.transactions_api();
    let internal_transactions = transactions_api
        .get_transaction_internal_txs(
            blockscout_client::apis::transactions_api::GetTransactionInternalTxsParams::builder()
                .transaction_hash(DEFAULT_TX_HASH.to_owned())
                .build(),
        )
        .await
        .expect("Failed to get transactions");
    let success_model = internal_transactions.try_parse_as_success_model().unwrap();
    assert!(!success_model.items.is_empty());
}

#[rstest]
#[tokio::test]
async fn smart_contracts(blockscout: Stubr) {
    let api_client = ApiClient::new(configuration(&blockscout));
    let smart_contracts_api = api_client.smart_contracts_api();
    let smart_contracts = smart_contracts_api
        .get_smart_contracts(
            blockscout_client::apis::smart_contracts_api::GetSmartContractsParams::builder()
                .build(),
        )
        .await
        .expect("Failed to get transactions");
    let success_model = smart_contracts.try_parse_as_success_model().unwrap();
    assert!(!success_model.items.is_empty());
    let _smart_contract = smart_contracts_api
        .get_smart_contract(
            blockscout_client::apis::smart_contracts_api::GetSmartContractParams::builder()
                .address_hash(DEFAULT_CONTRACT_HASH.to_owned())
                .build(),
        )
        .await
        .expect("Failed to get transactions");
}

#[rstest]
#[tokio::test]
async fn tokens(blockscout: Stubr) {
    let api_client = ApiClient::new(configuration(&blockscout));
    let tokens_api = api_client.tokens_api();
    let tokens = tokens_api
        .get_tokens_list(
            blockscout_client::apis::tokens_api::GetTokensListParams::builder().build(),
        )
        .await
        .expect("Failed to get transactions");
    let success_model = tokens.try_parse_as_success_model().unwrap();
    assert!(!success_model.items.is_empty());

    let _token = tokens_api
        .get_token(
            blockscout_client::apis::tokens_api::GetTokenParams::builder()
                .address_hash(DEFAULT_TOKEN_HASH.to_owned())
                .build(),
        )
        .await
        .expect("Failed to get transactions")
        .try_parse_as_success_model()
        .unwrap();
    let token_instances = tokens_api
        .get_nft_instances(
            blockscout_client::apis::tokens_api::GetNftInstancesParams::builder()
                .address_hash(DEFAULT_TOKEN_HASH.to_owned())
                .build(),
        )
        .await
        .expect("Failed to get transactions");
    let success_model = token_instances.try_parse_as_success_model().unwrap();
    assert!(!success_model.items.is_empty());

    let params = blockscout_client::apis::tokens_api::GetNftInstanceParams::builder()
        .address_hash(DEFAULT_TOKEN_HASH.to_owned())
        .id(DEFAULT_TOKEN_INSTANCE_NUMBER)
        .build();
    let _token_instance = tokens_api
        .get_nft_instance(params)
        .await
        .expect("Failed to get transactions")
        .try_parse_as_success_model()
        .unwrap();
}

#[rstest]
#[tokio::test]
async fn get_transaction(blockscout: Stubr) {
    let api_client = ApiClient::new(configuration(&blockscout));
    let transactions_api = api_client.transactions_api();
    let response = transactions_api
        .get_tx(
            blockscout_client::apis::transactions_api::GetTxParams::builder()
                .transaction_hash(DEFAULT_TX_HASH.to_owned())
                .build(),
        )
        .await
        .expect("Failed to get transactions");
    let success_model = response.try_parse_as_success_model().unwrap();
    assert_eq!(success_model.hash, DEFAULT_TX_HASH);
}

#[fixture]
fn blockscout() -> Stubr {
    Stubr::start_blocking("tests/recorded/eth_blockscout_com")
}

fn configuration(blockscout: &Stubr) -> Configuration {
    Configuration::new()
        .with_base_path(blockscout.uri().to_string())
        .to_owned()
}
