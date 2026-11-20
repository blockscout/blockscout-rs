use blockscout_service_launcher::test_database::TestDbGuard;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use uuid::Uuid;
use zetachain_cctx_entity::token;
use zetachain_cctx_logic::{database::ZetachainCctxDatabase, models::CrossChainTx};
mod helpers;

#[tokio::test]
async fn incoming_cctx_gas_token() {
    let cctx_json = json!(
        {
            "CrossChainTx": {
                "creator": "zeta15ruj2tc76pnj9xtw64utktee7cc7w6vzaes73z",
                "index": "0x00010947bf185e147e9d52e2b563cc9d62d103206ebbc4ce9a28cc495bf45dc2",
                "zeta_fees": "0",
                "relayed_message": "3e6f808e9e9ebfbb092e742d6329e5e41d8f0febd97b1de3619ed2c6beb3860147e30ca8a7dc98919a6ae1e6a4203154b3a927d9d22f96f9cdbd506a",
                "cctx_status": {
                    "status": "Aborted",
                    "status_message": "outTxGasFee(10109998000336000) more than available gas for tx (100000000000000) | Identifiers : 0x9a6AE1E6A4203154b3A927d9D22F96F9CDbD506a-80001-80001-0 : not enough gas",
                    "error_message": "",
                    "lastUpdate_timestamp": "1700464811",
                    "isAbortRefunded": false,
                    "created_timestamp": "0",
                    "error_message_revert": "",
                    "error_message_abort": ""
                },
                "inbound_params": {
                    "sender": "0x9a6AE1E6A4203154b3A927d9D22F96F9CDbD506a",
                    "sender_chain_id": "80001",
                    "tx_origin": "0x9a6AE1E6A4203154b3A927d9D22F96F9CDbD506a",
                    "coin_type": "Gas",
                    "asset": "",
                    "amount": "100000000000000",
                    "observed_hash": "0xabddb7d2d67320a3ef9033de705780ae02fbfc3cc05b2bdc711e05a26f059dac",
                    "observed_external_height": "42619610",
                    "ballot_index": "0x00010947bf185e147e9d52e2b563cc9d62d103206ebbc4ce9a28cc495bf45dc2",
                    "finalized_zeta_height": "2508324",
                    "tx_finalization_status": "NotFinalized",
                    "is_cross_chain_call": false,
                    "status": "SUCCESS",
                    "confirmation_mode": "SAFE"
                },
                "outbound_params": [
                    {
                        "receiver": "0x9a6AE1E6A4203154b3A927d9D22F96F9CDbD506a",
                        "receiver_chainId": "7001",
                        "coin_type": "Gas",
                        "amount": "0",
                        "tss_nonce": "0",
                        "gas_limit": "90000",
                        "gas_price": "",
                        "gas_priority_fee": "",
                        "hash": "0x09a3095eeef9e0287d033ebde9bb7b23c867013ec8375887ed81c2c67e7d9ff9",
                        "ballot_index": "",
                        "observed_external_height": "2508324",
                        "gas_used": "0",
                        "effective_gas_price": "0",
                        "effective_gas_limit": "0",
                        "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                        "tx_finalization_status": "NotFinalized",
                        "call_options": null,
                        "confirmation_mode": "SAFE"
                    },
                    {
                        "receiver": "0x9a6AE1E6A4203154b3A927d9D22F96F9CDbD506a",
                        "receiver_chainId": "80001",
                        "coin_type": "Gas",
                        "amount": "100000000000000",
                        "tss_nonce": "0",
                        "gas_limit": "90000",
                        "gas_price": "",
                        "gas_priority_fee": "",
                        "hash": "",
                        "ballot_index": "",
                        "observed_external_height": "0",
                        "gas_used": "0",
                        "effective_gas_price": "0",
                        "effective_gas_limit": "0",
                        "tss_pubkey": "",
                        "tx_finalization_status": "NotFinalized",
                        "call_options": null,
                        "confirmation_mode": "SAFE"
                    }
                ],
                "protocol_contract_version": "V1",
                "revert_options": {
                    "revert_address": "",
                    "call_on_revert": false,
                    "abort_address": "",
                    "revert_message": null,
                    "revert_gas_limit": "0"
                }
            }
        }
    );
    let cctx: CrossChainTx =
        serde_json::from_value(cctx_json.get("CrossChainTx").unwrap().clone()).unwrap();
    let cctx_token = zetachain_cctx_logic::models::Token {
        name: "dummy_token_1".to_string(),
        symbol: "DUMMY".to_string(),
        asset: "0x0000000000000000000000000000000000000001".to_string(),
        foreign_chain_id: "80001".to_string(),
        coin_type: zetachain_cctx_logic::models::CoinType::Gas,
        decimals: 18,
        gas_limit: "1000000000000000000".to_string(),
        paused: false,
        liquidity_cap: "1000000000000000000".to_string(),
        icon_url: None,
        zrc20_contract_address: Uuid::new_v4().to_string(),
    };
    let db = TestDbGuard::new::<migration::Migrator>("calculate_token").await;
    let database = ZetachainCctxDatabase::new(db.client(), 7001);
    database.setup_db().await.unwrap();
    database
        .sync_tokens(Uuid::new_v4(), vec![cctx_token.clone()])
        .await
        .unwrap();
    let expected_token_id = token::Entity::find()
        .filter(token::Column::Asset.eq(cctx_token.asset))
        .one(db.client().as_ref())
        .await
        .unwrap()
        .unwrap()
        .id;
    let token = database.calculate_token(&cctx).await.unwrap();

    assert_eq!(token.as_ref().map(|t| t.id), Some(expected_token_id));
}
