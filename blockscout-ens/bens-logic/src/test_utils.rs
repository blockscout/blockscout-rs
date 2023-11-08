use crate::subgraphs_reader::blockscout::BlockscoutClient;
use ethers::types::TxHash;
use std::collections::HashMap;
use wiremock::{
    matchers::{method, path},
    Mock, MockServer, ResponseTemplate,
};

lazy_static::lazy_static! {
    // executing this bash command for every transaction
    // curl https://eth.blockscout.com/api/v2/transactions/<tx_hash> | jq '. | {timestamp, "from": {"hash": .from["hash"]}, hash, method, block}'
    pub static ref TXNS: HashMap<TxHash, serde_json::Value> = {
        let txns = serde_json::json!({
            "0x09922ac0caf1efcc8f68ce004f382b46732258870154d8805707a1d4b098dfd0": {
                "timestamp": "2019-10-29T13:47:34.000000Z",
                "block": 8834378,
                "from": {
                    "hash": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
                },
                "hash": "0x09922ac0caf1efcc8f68ce004f382b46732258870154d8805707a1d4b098dfd0",
                "method": "setAddr"
            },
            "0xc3f86218c67bee8256b74b9b65d746a40bb5318a8b57948b804dbbbc3d0d7864": {
                "timestamp": "2020-02-06T18:23:40.000000Z",
                "block": 9430706,
                "from": {
                    "hash": "0x0904Dac3347eA47d208F3Fd67402D039a3b99859"
                },
                "hash": "0xc3f86218c67bee8256b74b9b65d746a40bb5318a8b57948b804dbbbc3d0d7864",
                "method": "migrateAll"
            },
            "0xea30bda97a7e9afcca208d5a648e8ec1e98b245a8884bf589dec8f4aa332fb14": {
                "timestamp": "2019-07-10T05:58:51.000000Z",
                "block": 8121770,
                "from": {
                    "hash": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
                },
                "hash": "0xea30bda97a7e9afcca208d5a648e8ec1e98b245a8884bf589dec8f4aa332fb14",
                "method": "transferRegistrars"
            },
            "0xdd16deb1ea750037c3ed1cae5ca20ff9db0e664a5146e5a030137d277a9247f3": {
                "timestamp": "2017-06-18T08:39:14.000000Z",
                "block": 3891899,
                "from": {
                    "hash": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
                },
                "hash": "0xdd16deb1ea750037c3ed1cae5ca20ff9db0e664a5146e5a030137d277a9247f3",
                "method": "finalizeAuction"
            },
            "0xbb13efab7f1f798f63814a4d184e903e050b38c38aa407f9294079ee7b3110c9": {
                "timestamp": "2021-02-15T17:19:17.000000Z",
                "block": 11862657,
                "from": {
                    "hash": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
                },
                "hash": "0xbb13efab7f1f798f63814a4d184e903e050b38c38aa407f9294079ee7b3110c9",
                "method": "setResolver"
            },
            "0x160ef4492c731ac6b59beebe1e234890cd55d4c556f8847624a0b47125fe4f84": {
                "timestamp": "2021-02-15T17:19:09.000000Z",
                "block": 11862656,
                "from": {
                    "hash": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
                },
                "hash": "0x160ef4492c731ac6b59beebe1e234890cd55d4c556f8847624a0b47125fe4f84",
                "method": "multicall"
            }
        }
        );


        serde_json::from_value(txns).unwrap()
    };
}

pub async fn mocked_blockscout_clients() -> HashMap<i64, BlockscoutClient> {
    let mock_server = MockServer::start().await;
    for (tx_hash, tx) in TXNS.iter() {
        let mock =
            Mock::given(method("GET")).and(path(&format!("/api/v2/transactions/{tx_hash:#x}")));
        mock.respond_with(ResponseTemplate::new(200).set_body_json(tx))
            .mount(&mock_server)
            .await;
    }
    let url = mock_server.uri().parse().unwrap();

    let client = BlockscoutClient::new(url, 1, 30);
    HashMap::from_iter([(1, client)])
}
