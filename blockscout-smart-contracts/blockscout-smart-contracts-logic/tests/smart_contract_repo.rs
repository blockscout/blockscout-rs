use std::collections::BTreeMap;
use blockscout_service_launcher::database;
use blockscout_smart_contracts_logic::create_input::CreateInput;
use blockscout_smart_contracts_logic::smart_contract_repo::{upsert_contract, select_contract, select_sources};
use migration::Migrator;

fn make_input(chain_id: &str, address_bytes: Vec<u8>, blockscout_url: &str, sources: &[(&str, &str)]) -> CreateInput {
    let mut map = BTreeMap::new();
    for (k, v) in sources {
        map.insert((*k).to_string(), (*v).to_string());
    }
    CreateInput {
        chain_id: chain_id.to_string(),
        address_bytes,
        blockscout_url: blockscout_url.to_string(),
        sources: map,
    }
}

#[tokio::test]
async fn upsert_contract_inserts_contract_and_sources() {
    let db_guard = database!(Migrator);

    let address_bytes = vec![
        0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
        0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0x00, 0x00, 0x00, 0x01,
    ];
    let input = make_input(
        "chain-A",
        address_bytes.clone(),
        "https://blockscout.com/a",
        &[("A.sol", "content-a"), ("B.sol", "content-b")],
    );

    upsert_contract(&db_guard.client(), &input).await.expect("upsert should succeed");

    // Assert contract was inserted
    let contract_opt = select_contract(db_guard.client().as_ref(), "chain-A", address_bytes.clone())
        .await
        .expect("select_contract should succeed");

    let contract = contract_opt.expect("contract should exist after upsert");
    assert_eq!(contract.chain_id, "chain-A");
    assert_eq!(contract.address_db, address_bytes);
    assert_eq!(contract.blockscout_url, "https://blockscout.com/a");

    // Assert sources were inserted
    let sources = select_sources(db_guard.client().as_ref(), contract.id)
        .await
        .expect("select_sources should succeed");

    assert_eq!(sources.len(), 2, "two source files should be inserted");
    assert_eq!(sources.get("A.sol"), Some(&"content-a".to_string()));
    assert_eq!(sources.get("B.sol"), Some(&"content-b".to_string()));
}
