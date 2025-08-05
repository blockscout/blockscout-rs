use blockscout_service_launcher::test_database::TestDbGuard;
use serde_json::json;
use uuid::Uuid;
use zetachain_cctx_logic::{database::ZetachainCctxDatabase, models::CrossChainTx};

mod helpers;

#[tokio::test]
async fn incoming_cctx_gas_token() {
    
    let cctx_json  = json!(
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
    let cctx: CrossChainTx = serde_json::from_value(cctx_json.get("CrossChainTx").unwrap().clone()).unwrap();
    let token = zetachain_cctx_logic::models::Token{
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
    database.sync_tokens(Uuid::new_v4(), vec![token]).await.unwrap();
    let token_id = database.calculate_token_id(&cctx).await.unwrap();
    assert_eq!(token_id, Some(1));
}

#[tokio::test]
async fn incoming_cctx_gas_token2() {
    
    let cctx_json  = json!(
        {
            "CrossChainTx": {
                "creator": "zeta167ns6zwczl9asjs47jwv3uhtkxfjcvx3dgf3ct",
                "index": "0x33ecf331db1623c545c09b4c10edb84312e8bfd73725dc475fd072bcc2cfbe6b",
                "zeta_fees": "0",
                "relayed_message": "",
                "cctx_status": {
                    "status": "OutboundMined",
                    "status_message": "",
                    "error_message": "",
                    "lastUpdate_timestamp": "1754427612",
                    "isAbortRefunded": false,
                    "created_timestamp": "1754427612",
                    "error_message_revert": "",
                    "error_message_abort": ""
                },
                "inbound_params": {
                    "sender": "0x08B9b0cE8657303fA078E13e2911db791D94f1D0",
                    "sender_chain_id": "11155111",
                    "tx_origin": "0x08B9b0cE8657303fA078E13e2911db791D94f1D0",
                    "coin_type": "Gas",
                    "asset": "0x0000000000000000000000000000000000000000",
                    "amount": "800000000000000",
                    "observed_hash": "0x4652dfb351de0db52c558e5b945febc458b566d6cb406917ce513bd0f3e74a61",
                    "observed_external_height": "8920631",
                    "ballot_index": "0x33ecf331db1623c545c09b4c10edb84312e8bfd73725dc475fd072bcc2cfbe6b",
                    "finalized_zeta_height": "11922281",
                    "tx_finalization_status": "Executed",
                    "is_cross_chain_call": false,
                    "status": "SUCCESS",
                    "confirmation_mode": "FAST"
                },
                "outbound_params": [
                    {
                        "receiver": "0x08B9b0cE8657303fA078E13e2911db791D94f1D0",
                        "receiver_chainId": "7001",
                        "coin_type": "Gas",
                        "amount": "0",
                        "tss_nonce": "0",
                        "gas_limit": "0",
                        "gas_price": "",
                        "gas_priority_fee": "",
                        "hash": "0xec550c0e400ec8069ae59ed9c4034929f76f3a39c81d1d804d28ba4514ee132a",
                        "ballot_index": "",
                        "observed_external_height": "11922281",
                        "gas_used": "0",
                        "effective_gas_price": "0",
                        "effective_gas_limit": "0",
                        "tss_pubkey": "zetapub1addwnpepq28c57cvcs0a2htsem5zxr6qnlvq9mzhmm76z3jncsnzz32rclangr2g35p",
                        "tx_finalization_status": "Executed",
                        "call_options": {
                            "gas_limit": "1500000",
                            "is_arbitrary_call": false
                        },
                        "confirmation_mode": "SAFE"
                    }
                ],
                "protocol_contract_version": "V2",
                "revert_options": {
                    "revert_address": "0x08B9b0cE8657303fA078E13e2911db791D94f1D0",
                    "call_on_revert": false,
                    "abort_address": "0x0000000000000000000000000000000000000000",
                    "revert_message": null,
                    "revert_gas_limit": "0"
                }
            }
        }
    );
    let cctx: CrossChainTx = serde_json::from_value(cctx_json.get("CrossChainTx").unwrap().clone()).unwrap();
    let token = zetachain_cctx_logic::models::Token{
        name: "dummy_token_2".to_string(),
        symbol: "DUMMY2".to_string(),
        asset: "0x0000000000000000000000000000000000000002".to_string(),
        foreign_chain_id: "11155111".to_string(),
        coin_type: zetachain_cctx_logic::models::CoinType::Gas,
        decimals: 18,
        gas_limit: "1000000000000000000".to_string(),
        paused: false,
        liquidity_cap: "1000000000000000000".to_string(),
        icon_url: None,
        zrc20_contract_address: Uuid::new_v4().to_string(),
    };
    let db = TestDbGuard::new::<migration::Migrator>("calculate_token2").await;
    let database = ZetachainCctxDatabase::new(db.client(), 7001);
    database.sync_tokens(Uuid::new_v4(), vec![token]).await.unwrap();
    let token_id = database.calculate_token_id(&cctx).await.unwrap();
    assert_eq!(token_id, Some(1));
}

#[tokio::test]
async fn incoming_cctx_gas_token3() {
    
    let cctx_json  = json!(
        
        {
            "CrossChainTx": {
                "creator": "zeta1mte0r3jzkf2rkd7ex4p3xsd3fxqg7q29q0wxl5",
                "index": "0x0639be1d8aaace6ab1a6aff1dea7a5035cb6ae633f95495e30fa183ad5f78942",
                "zeta_fees": "0",
                "relayed_message": "",
                "cctx_status": {
                    "status": "OutboundMined",
                    "status_message": "",
                    "error_message": "",
                    "lastUpdate_timestamp": "1754426145",
                    "isAbortRefunded": false,
                    "created_timestamp": "1754426145",
                    "error_message_revert": "",
                    "error_message_abort": ""
                },
                "inbound_params": {
                    "sender": "tb1qhamwhl4w4sdv4j6g5vsfntna2a80kvkunllytl",
                    "sender_chain_id": "18333",
                    "tx_origin": "tb1qhamwhl4w4sdv4j6g5vsfntna2a80kvkunllytl",
                    "coin_type": "Gas",
                    "asset": "",
                    "amount": "998368",
                    "observed_hash": "8dcb14c4793a263cdee154b5b9713cf094dd0fcd30fb2cf3ecec1f8b0ea7560c",
                    "observed_external_height": "263817",
                    "ballot_index": "0x0639be1d8aaace6ab1a6aff1dea7a5035cb6ae633f95495e30fa183ad5f78942",
                    "finalized_zeta_height": "11921939",
                    "tx_finalization_status": "Executed",
                    "is_cross_chain_call": false,
                    "status": "SUCCESS",
                    "confirmation_mode": "SAFE"
                },
                "outbound_params": [
                    {
                        "receiver": "0x58A8Ba18c585C411B95Ba1e78962a2A3E1c6f52a",
                        "receiver_chainId": "7001",
                        "coin_type": "Gas",
                        "amount": "0",
                        "tss_nonce": "0",
                        "gas_limit": "0",
                        "gas_price": "",
                        "gas_priority_fee": "",
                        "hash": "0x3ceeaaa3ac189954b5b3c6316f1288ed85ba7c8e619f5dce044768c6a530dedd",
                        "ballot_index": "",
                        "observed_external_height": "11921939",
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
                    }
                ],
                "protocol_contract_version": "V2",
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
    let cctx: CrossChainTx = serde_json::from_value(cctx_json.get("CrossChainTx").unwrap().clone()).unwrap();
    let token = zetachain_cctx_logic::models::Token{
        name: "dummy_token_2".to_string(),
        symbol: "DUMMY3".to_string(),
        asset: "0x0000000000000000000000000000000000000002".to_string(),
        foreign_chain_id: "18333".to_string(),
        coin_type: zetachain_cctx_logic::models::CoinType::Gas,
        decimals: 18,
        gas_limit: "1000000000000000000".to_string(),
        paused: false,
        liquidity_cap: "1000000000000000000".to_string(),
        icon_url: None,
        zrc20_contract_address: Uuid::new_v4().to_string(),
    };
    let db = TestDbGuard::new::<migration::Migrator>("calculate_token3").await;
    let database = ZetachainCctxDatabase::new(db.client(), 7001);
    database.sync_tokens(Uuid::new_v4(), vec![token]).await.unwrap();
    let token_id = database.calculate_token_id(&cctx).await.unwrap();
    assert_eq!(token_id, Some(1));
}