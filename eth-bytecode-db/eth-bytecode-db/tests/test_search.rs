#![cfg(feature = "test-utils")]

use blockscout_service_launcher::test_database::TestDbGuard;
use entity::{sea_orm_active_enums::BytecodeType, sources};
use eth_bytecode_db::{
    search::{find_contract, BytecodeRemote},
    tests::verifier_mock::{
        generate_and_insert, BytecodePart, ContractInfo, ContractType, PartTy, VerificationResult,
    },
    verification::MatchType,
};
use sea_orm::DatabaseConnection;
use std::{collections::HashMap, str::FromStr};

async fn prepare_db(
    db: &DatabaseConnection,
    max_id: usize,
) -> HashMap<ContractInfo, entity::sources::Model> {
    let mut all_sources = HashMap::new();
    for i in 1..max_id {
        for ty in [
            ContractType::Small,
            ContractType::Medium,
            ContractType::Big,
            ContractType::Constructor,
        ] {
            let info = ContractInfo { id: i, ty };
            let source = generate_and_insert(db, &info)
                .await
                .expect("cannot push contract");
            all_sources.insert(info, source);
        }
    }
    all_sources
}

fn get_raw_creation_bytecode(verification_result: &VerificationResult, change: bool) -> String {
    let mut raw_creation_input = verification_result
        .local_creation_input_parts
        .iter()
        .map(|p| {
            if change {
                change_part_for_search(p)
            } else {
                p.data.trim_start_matches("0x").to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("");

    match &verification_result.constructor_arguments {
        Some(args) => raw_creation_input.push_str(args.trim_start_matches("0x")),
        None => {}
    };

    raw_creation_input
}

fn change_part_for_search(part: &BytecodePart) -> String {
    let changed = match part.r#type {
        PartTy::Main => &part.data,
        PartTy::Meta => {
            let n = part.data.len();
            let metadata_length = &part.data[n - 4..];
            match metadata_length {
                "0033" => "a2646970667358221220c424331e61ba143d01f757e1a3b6ddcfe99698f6c1862e2133c4d7d277854b9564736f6c63430008070033",
                "0032" => "a265627a7a72315820a648f0e3107b949c9f7567adacfd4b276c9fc37dc06b172c7efbd1a0e58206ce64736f6c63430005110032",
                "0029" => "a165627a7a72305820a61b515152276dcea013aa8566142e7d3f07992c7c9512373cc7ba9a33fc2eab0029",
                _ => panic!("unknown metadata length '{metadata_length}', add this type of metadata to mock")
            }
        }
    };
    changed.trim_start_matches("0x").to_string()
}

async fn check_bytecode_search(
    db: &DatabaseConnection,
    contract_info: ContractInfo,
    expected_source: &sources::Model,
    expected_contract: &VerificationResult,
    raw_remote_bytecode: &str,
    bytecode_type: BytecodeType,
    match_type: MatchType,
) {
    let data = blockscout_display_bytes::Bytes::from_str(raw_remote_bytecode)
        .unwrap()
        .0;
    let search = BytecodeRemote {
        data,
        bytecode_type,
    };
    let partial_matches = find_contract(db, &search)
        .await
        .expect("error during contract search");

    assert_eq!(
        partial_matches.len(),
        1,
        "contract not found. info={contract_info:?}"
    );
    let contract = partial_matches
        .into_iter()
        .next()
        .expect("checked that len is 1");

    assert_eq!(&contract.contract_name, &expected_source.contract_name);
    assert_eq!(
        contract.constructor_arguments,
        expected_contract
            .constructor_arguments
            .clone()
            .map(|args| args.trim_start_matches("0x").to_string())
    );
    assert_eq!(contract.match_type, match_type);
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_full_match_search_bytecodes() {
    let db = TestDbGuard::new::<migration::Migrator>("test_full_match_search_bytecodes")
        .await
        .client();
    let max_id = 10;
    let change_bytecode = false;
    let all_sources = prepare_db(&db, max_id).await;

    for id in 1..max_id {
        for ty in [
            ContractType::Small,
            ContractType::Medium,
            ContractType::Big,
            ContractType::Constructor,
        ] {
            let info = ContractInfo { id, ty };
            let expected_source = all_sources.get(&info).expect("source should be in hashmap");
            let expected_contract = VerificationResult::generate(&info);
            let raw_creation_input = get_raw_creation_bytecode(&expected_contract, change_bytecode);
            check_bytecode_search(
                &db,
                info,
                expected_source,
                &expected_contract,
                &raw_creation_input,
                BytecodeType::CreationInput,
                MatchType::Full,
            )
            .await;
        }
    }
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_partial_search_bytecodes() {
    let db = TestDbGuard::new::<migration::Migrator>("test_partial_search_bytecodes")
        .await
        .client();
    let max_id = 10;
    let repeated_amount = 10;
    let repeated_info = ContractInfo {
        id: 77777,
        ty: ContractType::Small,
    };
    let change_bytecode = true;
    let mut all_sources = prepare_db(&db, max_id).await;
    for _ in 0..repeated_amount {
        let source = generate_and_insert(&db, &repeated_info)
            .await
            .expect("cannot push contract");
        all_sources.insert(repeated_info, source);
    }

    // Search known bytecodes
    for id in 1..max_id {
        for ty in [
            ContractType::Small,
            ContractType::Medium,
            ContractType::Big,
            ContractType::Constructor,
        ] {
            let info = ContractInfo { id, ty };
            let expected_source = all_sources.get(&info).expect("source should be in hashmap");
            let expected_contract = VerificationResult::generate(&info);
            // Get bytecode from verification result, and change it
            let raw_creation_input = get_raw_creation_bytecode(&expected_contract, change_bytecode);
            check_bytecode_search(
                &db,
                info,
                expected_source,
                &expected_contract,
                &raw_creation_input,
                BytecodeType::CreationInput,
                MatchType::Partial,
            )
            .await;
        }
    }

    // Search repeated bytecodes
    let expected_source = all_sources
        .get(&repeated_info)
        .expect("source should be in hashmap");
    let expected_contract = VerificationResult::generate(&repeated_info);

    let raw_creation_input = get_raw_creation_bytecode(&expected_contract, change_bytecode);

    let data = blockscout_display_bytes::Bytes::from_str(&raw_creation_input)
        .unwrap()
        .0;
    let search = BytecodeRemote {
        data,
        bytecode_type: BytecodeType::CreationInput,
    };
    let partial_matches = find_contract(db.as_ref(), &search)
        .await
        .expect("error during contract search");
    assert_eq!(partial_matches.len(), repeated_amount);
    for contract in partial_matches {
        assert_eq!(&contract.contract_name, &expected_source.contract_name);
        assert_eq!(
            contract.constructor_arguments,
            expected_contract
                .clone()
                .constructor_arguments
                .map(|args| args.trim_start_matches("0x").to_string())
        );
        assert_eq!(contract.match_type, MatchType::Partial);
    }

    // Search unknow bytecodes
    for id in max_id + 10..max_id + 20 {
        for ty in [
            ContractType::Small,
            ContractType::Medium,
            ContractType::Big,
            ContractType::Constructor,
        ] {
            let info = ContractInfo { id, ty };
            let unknow_contract = VerificationResult::generate(&info);
            let raw_creation_input = get_raw_creation_bytecode(&unknow_contract, change_bytecode);
            let data = blockscout_display_bytes::Bytes::from_str(&raw_creation_input)
                .unwrap()
                .0;
            let search = BytecodeRemote {
                data,
                bytecode_type: BytecodeType::CreationInput,
            };

            let partial_matches = find_contract(db.as_ref(), &search)
                .await
                .expect("unkown contract should not give error");
            assert!(
                partial_matches.is_empty(),
                "found some contact, but bytecode is unknow"
            );
        }
    }

    // Search random strings
    for bytecode in ["", "6080", "0000", "1111"] {
        let data = blockscout_display_bytes::Bytes::from_str(bytecode)
            .unwrap()
            .0;
        let search = BytecodeRemote {
            data,
            bytecode_type: BytecodeType::CreationInput,
        };

        let partial_matches = find_contract(db.as_ref(), &search)
            .await
            .expect("random string should not give error");
        assert!(
            partial_matches.is_empty(),
            "found some contact, but bytecode is random string"
        );
    }
}
