#![allow(dead_code)]
mod helpers;
mod test_db;

use blockscout_service_launcher::{database, test_server};
use multichain_aggregator_logic::types::api_keys::ApiKey;
use multichain_aggregator_proto::blockscout::multichain_aggregator::v1 as proto;
use sea_orm::prelude::Uuid;

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_list_nfts() {
    let db = database!(test_db::TestMigrator);

    helpers::upsert_api_keys(
        db.client().as_ref(),
        vec![ApiKey {
            key: Uuid::new_v4(),
            chain_id: 1,
        }],
    )
    .await
    .unwrap();

    let base = helpers::init_server(db.db_url()).await;

    let validate_nft = |item: &proto::Address| {
        matches!(
            item.token_type(),
            proto::TokenType::Erc1155 | proto::TokenType::Erc721
        )
    };

    let response: proto::ListNftsResponse =
        test_server::send_get_request(&base, "/api/v1/nfts?q=test&chain_id=1&page_size=20").await;

    assert_eq!(response.items.len(), 20);
    assert!(response.items.iter().all(validate_nft));

    let page_token = response.next_page_params.unwrap().page_token;
    let response: proto::ListNftsResponse = test_server::send_get_request(
        &base,
        &format!("/api/v1/nfts?q=test&chain_id=1&page_size=20&page_token={page_token}"),
    )
    .await;

    assert_eq!(response.items.len(), 4);
    assert!(response.next_page_params.is_none());
    assert!(response.items.iter().all(validate_nft));
}
