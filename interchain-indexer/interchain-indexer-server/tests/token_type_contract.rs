// SPDX-License-Identifier: LicenseRef-Blockscout

mod helpers;

use blockscout_service_launcher::test_server;
use chrono::Utc;
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{MessageStatus, TokenType},
    tokens,
};
use sea_orm::{ActiveValue::Set, EntityTrait, prelude::BigDecimal};

#[tokio::test]
#[ignore = "Needs database to run"]
async fn read_api_token_type_comes_from_tokens_and_hides_native_sentinel() {
    let db = helpers::init_db("test", "read_api_token_types").await;
    let base = helpers::init_interchain_indexer_server(db.db_url(), |settings| settings).await;
    let conn = db.client();

    for (chain_id, address, token_type, symbol) in [
        (1, vec![0x11; 20], TokenType::Erc20, "DAI"),
        (100, vec![0; 20], TokenType::Native, "xDAI"),
    ] {
        tokens::Entity::insert(tokens::ActiveModel {
            chain_id: Set(chain_id),
            address: Set(address),
            r#type: Set(token_type),
            name: Set(Some(symbol.to_string())),
            symbol: Set(Some(symbol.to_string())),
            decimals: Set(Some(18)),
            updated_at: Set(Some(Utc::now().naive_utc())),
            ..Default::default()
        })
        .exec(conn.as_ref())
        .await
        .unwrap();
    }

    crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
        id: Set(8888),
        bridge_id: Set(1),
        status: Set(MessageStatus::Completed),
        init_timestamp: Set(Utc::now().naive_utc()),
        src_chain_id: Set(1),
        dst_chain_id: Set(Some(100)),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();
    crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
        message_id: Set(8888),
        bridge_id: Set(1),
        index: Set(0),
        token_src_chain_id: Set(1),
        token_dst_chain_id: Set(100),
        token_src_address: Set(Some(vec![0x11; 20])),
        token_dst_address: Set(Some(vec![0; 20])),
        src_amount: Set(Some(BigDecimal::from(123))),
        dst_amount: Set(Some(BigDecimal::from(123))),
        ..Default::default()
    })
    .exec(conn.as_ref())
    .await
    .unwrap();

    let details: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/messages/0x22b8?bridge_id=1")
            .await;
    let transfers: serde_json::Value =
        test_server::send_get_request(&base, "/api/v1/interchain/transfers").await;
    for transfer in [&details["transfers"][0], &transfers["items"][0]] {
        assert_eq!(transfer["source_token"]["type"], "ERC20");
        assert_eq!(
            transfer["source_token"]["address_hash"],
            "0x1111111111111111111111111111111111111111"
        );
        assert_eq!(transfer["destination_token"]["type"], "NATIVE");
        assert_eq!(
            transfer["destination_token"].get("address_hash"),
            Some(&serde_json::Value::Null)
        );
        assert_eq!(transfer["destination_token"]["symbol"], "xDAI");
        assert_eq!(transfer["destination_token"]["decimals"], "18");
    }
}
