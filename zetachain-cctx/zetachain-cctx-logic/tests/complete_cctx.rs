mod helpers;

use serde_json::json;
use zetachain_cctx_logic::database::ZetachainCctxDatabase;
use zetachain_cctx_logic::models::CrossChainTx;
use uuid::Uuid;
use migration::sea_orm::TransactionTrait;

#[tokio::test]
async fn test_get_complete_cctx() {
    let db = crate::helpers::init_db("test", "indexer_get_complete_cctx").await;
    let database = ZetachainCctxDatabase::new(db.client().clone(), 7001);
    database.setup_db().await.unwrap();

    let cctx_response = json!({
        "CrossChainTx": {
            "creator": "zeta1dxyzsket66vt886ap0gnzlnu5pv0y99v086wnz",
            "index": "0x230d3138bf679c985b114ad3fef2b3eeb9a0d52852e84f67c601ffbdda776a01",
            "zeta_fees": "0",
            "relayed_message": "000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000076465706f73697400000000000000000000000000000000000000000000000000",
            "cctx_status": {
                "status": "Aborted",
                "status_message": "revert failed to be processed",
                "error_message": "{\"type\":\"contract_call_error\",\"message\":\"contract call failed when calling EVM with data\",\"error\":\"execution reverted: ret 0x: evm transaction execution failed\",\"method\":\"depositAndCall0\",\"contract\":\"0x6c533f7fE93fAE114d0954697069Df33C9B74fD7\",\"args\":\"[{[116 98 49 113 117 101 103 109 57 108 103 54 110 100 48 118 50 120 110 99 108 56 108 100 107 118 102 107 103 104 104 101 56 109 110 115 51 102 116 118 99 97] 0x6C646b76666b67686865386D6E73336674766361 18333} 0xdbfF6471a79E5374d771922F2194eccc42210B9F 236 0x1607A220D52FeB7c6689e934E47B4b0864B2DD90 [0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 32 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 7 100 101 112 111 115 105 116 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0]]\"}",
                "lastUpdate_timestamp": "1754299496",
                "isAbortRefunded": false,
                "created_timestamp": "1754299496",
                "error_message_revert": "{\"type\":\"internal_error\",\"error\":\"unable to pay for outbound tx using gas token, outbound chain: 18333, required: 2200, available: 236: not enough gas\"}",
                "error_message_abort": "abort processing not supported for this cctx"
            },
            "inbound_params": {
                "sender": "tb1quegm9lg6nd0v2xncl8ldkvfkghhe8mns3ftvca",
                "sender_chain_id": "18333",
                "tx_origin": "tb1quegm9lg6nd0v2xncl8ldkvfkghhe8mns3ftvca",
                "coin_type": "Gas",
                "asset": "",
                "amount": "236",
                "observed_hash": "8cd4a965fa23ba7cb6f77e91628ffe4c2952df5040c6098af4b7f07df2ba3318",
                "observed_external_height": "263597",
                "ballot_index": "0x230d3138bf679c985b114ad3fef2b3eeb9a0d52852e84f67c601ffbdda776a01",
                "finalized_zeta_height": "11892444",
                "tx_finalization_status": "Executed",
                "is_cross_chain_call": true,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "0x1607A220D52FeB7c6689e934E47B4b0864B2DD90",
                    "receiver_chainId": "7001",
                    "coin_type": "Gas",
                    "amount": "0",
                    "tss_nonce": "0",
                    "gas_limit": "0",
                    "gas_price": "",
                    "gas_priority_fee": "",
                    "hash": "0x8c1c9db864b8aebf018c3c91ae2a3d5e23a1007bf9205f83968fa072a48cdaa1",
                    "ballot_index": "",
                    "observed_external_height": "11892444",
                    "gas_used": "0",
                    "effective_gas_price": "0",
                    "effective_gas_limit": "0",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "Executed",
                    "call_options": {
                        "gas_limit": "0",
                        "is_arbitrary_call": false
                    },
                    "confirmation_mode": "SAFE"
                },
                {
                    "receiver": "tb1quegm9lg6nd0v2xncl8ldkvfkghhe8mns3ftvca",
                    "receiver_chainId": "18333",
                    "coin_type": "Gas",
                    "amount": "0",
                    "tss_nonce": "0",
                    "gas_limit": "0",
                    "gas_price": "",
                    "gas_priority_fee": "",
                    "hash": "",
                    "ballot_index": "",
                    "observed_external_height": "0",
                    "gas_used": "0",
                    "effective_gas_price": "0",
                    "effective_gas_limit": "0",
                    "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                    "tx_finalization_status": "NotFinalized",
                    "call_options": {
                        "gas_limit": "100",
                        "is_arbitrary_call": false
                    },
                    "confirmation_mode": "SAFE"
                }
            ],
            "protocol_contract_version": "V2",
            "revert_options": {
                "revert_address": "tb1quegm9lg6nd0v2xncl8ldkvfkghhe8mns3ftvca",
                "call_on_revert": false,
                "abort_address": "",
                "revert_message": null,
                "revert_gas_limit": "0"
            }
        }
    });

    let token = helpers::dummy_token("BTC", "sBTC.BTC", None, "18333", zetachain_cctx_logic::models::CoinType::Gas);
    database.sync_tokens(Uuid::new_v4(), vec![token]).await.unwrap();
    let cctx = cctx_response.get("CrossChainTx").unwrap();
    let cctx: CrossChainTx = serde_json::from_value(cctx.clone()).unwrap();
    let tx = db.client().begin().await.unwrap();
    database.batch_insert_transactions(Uuid::new_v4(), &vec![cctx], &tx).await.unwrap();
    tx.commit().await.unwrap();
    let cctx = database.get_complete_cctx("0x230d3138bf679c985b114ad3fef2b3eeb9a0d52852e84f67c601ffbdda776a01".to_string()).await.unwrap();
    assert!(cctx.is_some());
    let cctx = cctx.unwrap();
    assert_eq!(cctx.index, "0x230d3138bf679c985b114ad3fef2b3eeb9a0d52852e84f67c601ffbdda776a01");
    assert_eq!(cctx.creator, "zeta1dxyzsket66vt886ap0gnzlnu5pv0y99v086wnz");
    assert_eq!(cctx.zeta_fees, "0");
    assert_eq!(cctx.relayed_message, "000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000076465706f73697400000000000000000000000000000000000000000000000000");
    assert_eq!(cctx.token_symbol, Some("sBTC.BTC".to_string()));
    
}