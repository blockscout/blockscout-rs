use blockscout_service_launcher::test_database::TestDbGuard;
use uuid::Uuid;
use zetachain_cctx_logic::{database::ZetachainCctxDatabase, models::PagedCCTXResponse};
use migration::sea_orm::TransactionTrait;

const BAD_CCTX: &str = r#"
{
    "CrossChainTx": [
        {
            "creator": "zeta1w8qa37h22h884vxedmprvwtd3z2nwakxu9k935",
            "index": "0x00c79475d70a986045a5376e4931fcf1383e836c19fc71443a6348a276a76d8b",
            "zeta_fees": "0",
            "relayed_message": "44d1f1f9289dba1cf5824bd667184cebe020aa1c00000000000000000000000048f80608b672dc30dc7e3dbbd0343c5f02c738eb00000000000000000000000005f9b1c2aeb00ded6ea826d0dc1779e6227fce520000000000000000000000000000000000000000000000000000000000000000",
            "cctx_status": {
                "status": "OutboundMined",
                "status_message": "Remote omnichain contract call completed",
                "error_message": "",
                "lastUpdate_timestamp": "1693417780",
                "isAbortRefunded": false,
                "created_timestamp": "0",
                "error_message_revert": "",
                "error_message_abort": ""
            },
            "inbound_params": {
                "sender": "0x05f9b1c2AeB00DEd6Ea826d0DC1779E6227fCe52",
                "sender_chain_id": "5",
                "tx_origin": "0x05f9b1c2AeB00DEd6Ea826d0DC1779E6227fCe52",
                "coin_type": "Gas",
                "asset": "",
                "amount": "18000000000000000",
                "observed_hash": "0xeab8fc8c0298308be5253595ae2b71a646f517957406adae7089769386d9387f",
                "observed_external_height": "9608098",
                "ballot_index": "0x00c79475d70a986045a5376e4931fcf1383e836c19fc71443a6348a276a76d8b",
                "finalized_zeta_height": "1406248",
                "tx_finalization_status": "NotFinalized",
                "is_cross_chain_call": false,
                "status": "SUCCESS",
                "confirmation_mode": "SAFE"
            },
            "outbound_params": [
                {
                    "receiver": "0x05f9b1c2AeB00DEd6Ea826d0DC1779E6227fCe52",
                    "receiver_chainId": "7001",
                    "coin_type": "Gas",
                    "amount": "0",
                    "tss_nonce": "0",
                    "gas_limit": "90000",
                    "gas_price": "",
                    "gas_priority_fee": "",
                    "hash": "0xf12ddfcb1f223b3ff0bb5563b3d99830398efa81fd1ebfde64048ac034feda33",
                    "ballot_index": "",
                    "observed_external_height": "1406248",
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
    ],
    "pagination": {
        "next_key": "MHgwMGM3OTY3NzBjOGQwMjdkMmM3YWNmYTMwYzZkYjBkNDAyMzhkYjIxZjFlNzc3NTRlMWU5MzMyN2M4OTM3MmZm",
        "total": "0"
    }
}
"#;

#[tokio::test]
async fn test_batch_insert(){

 let response: PagedCCTXResponse = serde_json::from_str(BAD_CCTX).unwrap();

 let db = TestDbGuard::new::<migration::Migrator>("batch_insert").await;

 let tx = db.client().begin().await.unwrap();

 let cctxs = response.cross_chain_tx;

 let job_id = Uuid::new_v4();

 let database = ZetachainCctxDatabase::new(db.client());

 let res = database.batch_insert_transactions(job_id, &cctxs, &tx).await;

 assert!(res.is_ok());

 tx.commit().await.unwrap();



}