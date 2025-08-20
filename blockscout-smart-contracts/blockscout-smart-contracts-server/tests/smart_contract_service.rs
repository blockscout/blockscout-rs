use blockscout_smart_contracts_proto::blockscout::blockscout_smart_contracts::v1::{SmartContractServiceCreateRequest, SmartContract, SmartContractServiceGetRequest};
use blockscout_smart_contracts_server::services::SmartContractServiceImpl;
use blockscout_service_launcher::database;
use blockscout_smart_contracts_logic::{
    address_utils::parse_address_to_bytes,
    smart_contract_repo::{select_contract, select_sources},
};
use migration::Migrator;
use std::collections::BTreeMap;
use tonic::Request;
use blockscout_smart_contracts_proto::blockscout::blockscout_smart_contracts::v1::smart_contract_service_server::SmartContractService;

#[tokio::test]
async fn smart_contract_service_create_inserts_contract_and_sources() {
    // Arrange: init DB and service
    let db_guard = database!(Migrator);
    let service = SmartContractServiceImpl {
        db: db_guard.client(),
    };

    // Prepare request payload
    let address_hex = "0xdeadbeefdeadbeefdeadbeefdeadbeef00000001";
    let mut sources = BTreeMap::new();
    sources.insert("A.sol".to_string(), "content-a".to_string());
    sources.insert("B.sol".to_string(), "content-b".to_string());

    let req = SmartContractServiceCreateRequest {
        contract: Some(SmartContract {
            chain_id: "chain-A".to_string(),
            address: address_hex.to_string(),
            blockscout_url: "https://blockscout.com/a".to_string(),
            sources,
        }),
    };

    // Act: call the gRPC method
    let result = service
        .smart_contract_service_create(Request::new(req))
        .await;

    // Assert: call succeeded
    assert!(result.is_ok(), "smart_contract_service_create should succeed");

    // Assert: contract and sources were persisted
    let addr_bytes = parse_address_to_bytes(address_hex).expect("valid hex address");
    let contract_opt =
        select_contract(service.db.as_ref(), "chain-A", addr_bytes.clone())
            .await
            .expect("select_contract should succeed");
    let contract = contract_opt.expect("contract should exist after create");
    assert_eq!(contract.chain_id, "chain-A");
    assert_eq!(contract.address_db, addr_bytes);
    assert_eq!(contract.blockscout_url, "https://blockscout.com/a");

    let persisted_sources =
        select_sources(service.db.as_ref(), contract.id)
            .await
            .expect("select_sources should succeed");
    assert_eq!(persisted_sources.len(), 2);
    assert_eq!(persisted_sources.get("A.sol"), Some(&"content-a".to_string()));
    assert_eq!(persisted_sources.get("B.sol"), Some(&"content-b".to_string()));
}

#[tokio::test]
async fn smart_contract_service_get_returns_none_for_absent_contract() {
    // Arrange
    let db_guard = database!(Migrator);
    let service = SmartContractServiceImpl {
        db: db_guard.client(),
    };

    // Act
    let resp = service
        .smart_contract_service_get(Request::new(SmartContractServiceGetRequest {
            chain_id: "chain-Z".to_string(),
            address: "0xdeadbeefdeadbeefdeadbeefdeadbeef00000002".to_string(),
        }))
        .await
        .expect("get should return Ok result")
        .into_inner();

    // Assert
    assert!(
        resp.contract.is_none(),
        "Expected None for non-existent contract"
    );
}

#[tokio::test]
async fn smart_contract_service_get_returns_stored_contract() {
    // Arrange
    let db_guard = database!(Migrator);
    let service = SmartContractServiceImpl {
        db: db_guard.client(),
    };

    let address_hex = "0xdeadbeefdeadbeefdeadbeefdeadbeef00000001";
    let mut sources = BTreeMap::new();
    sources.insert("A.sol".to_string(), "content-a".to_string());
    sources.insert("B.sol".to_string(), "content-b".to_string());

    // Insert a contract via the create endpoint
    service
        .smart_contract_service_create(Request::new(SmartContractServiceCreateRequest {
            contract: Some(SmartContract {
                chain_id: "chain-A".to_string(),
                address: address_hex.to_string(),
                blockscout_url: "https://blockscout.com/a".to_string(),
                sources: sources.clone(),
            }),
        }))
        .await
        .expect("create should succeed");

    // Act: fetch it via get
    let resp = service
        .smart_contract_service_get(Request::new(SmartContractServiceGetRequest {
            chain_id: "chain-A".to_string(),
            address: address_hex.to_string(),
        }))
        .await
        .expect("get should succeed")
        .into_inner();

    // Assert: contract is present and matches
    let contract = resp.contract.expect("contract should be returned");
    assert_eq!(contract.chain_id, "chain-A");
    // The service formats the address based on stored bytes and input format, which should match the requested format
    assert_eq!(contract.address, address_hex);
    assert_eq!(contract.blockscout_url, "https://blockscout.com/a");
    assert_eq!(contract.sources.len(), 2);
    assert_eq!(contract.sources.get("A.sol"), Some(&"content-a".to_string()));
    assert_eq!(contract.sources.get("B.sol"), Some(&"content-b".to_string()));
}
