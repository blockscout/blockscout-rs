#![allow(dead_code)]
mod helpers;

use blockscout_service_launcher::{database, test_server};
use migration::Migrator;
use multichain_aggregator_logic::types::api_keys::ApiKey;
use multichain_aggregator_proto::blockscout::multichain_aggregator::v1 as proto;
use pretty_assertions::assert_eq;
use sea_orm::prelude::Uuid;

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_fetch_chains() {
    let db = database!(Migrator);

    helpers::create_test_chains(&db, 3).await;
    helpers::upsert_api_keys(
        db.client().as_ref(),
        [2, 3]
            .into_iter()
            .map(|id| ApiKey {
                key: Uuid::new_v4(),
                chain_id: id,
            })
            .collect(),
    )
    .await
    .unwrap();

    let base = helpers::init_server(db.db_url()).await;

    let response: proto::ListChainsResponse =
        test_server::send_get_request(&base, "/api/v1/chains?only_active=true").await;
    assert_eq!(
        response.items,
        vec![
            proto::Chain {
                id: "2".to_string(),
                name: "Chain 2".to_string(),
                explorer_url: "https://test2".to_string(),
                icon_url: "https://test2".to_string(),
            },
            proto::Chain {
                id: "3".to_string(),
                name: "Chain 3".to_string(),
                explorer_url: "https://test3".to_string(),
                icon_url: "https://test3".to_string(),
            },
        ]
    );

    let response: proto::ListChainsResponse =
        test_server::send_get_request(&base, "/api/v1/chains").await;
    let mut chains = response.items;
    chains.sort_by_key(|c| c.id.clone());
    assert_eq!(
        chains,
        vec![
            proto::Chain {
                id: "1".to_string(),
                name: "Chain 1".to_string(),
                explorer_url: "https://test1".to_string(),
                icon_url: "https://test1".to_string(),
            },
            proto::Chain {
                id: "2".to_string(),
                name: "Chain 2".to_string(),
                explorer_url: "https://test2".to_string(),
                icon_url: "https://test2".to_string(),
            },
            proto::Chain {
                id: "3".to_string(),
                name: "Chain 3".to_string(),
                explorer_url: "https://test3".to_string(),
                icon_url: "https://test3".to_string(),
            },
        ]
    );
}
