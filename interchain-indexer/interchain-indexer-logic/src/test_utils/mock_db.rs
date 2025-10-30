use sea_orm::{ActiveValue::Set, DatabaseConnection, EntityTrait};
use interchain_indexer_entity::{chains, bridges, bridge_contracts, crosschain_messages, crosschain_transfers};
use alloy::primitives::{address, Address};

pub async fn fill_mock_interchain_database(db: &DatabaseConnection) {
    chains::Entity::insert_many([
        chains::ActiveModel {
            id: Set(1),
            name: Set("Ethereum".to_string()),
            ..Default::default()
        },
        chains::ActiveModel {
            id: Set(100),
            name: Set("Gnosis".to_string()),
            ..Default::default()
        },
    ])
    .exec(db)
    .await
    .unwrap();

    bridges::Entity::insert_many([
        bridges::ActiveModel {
            id: Set(1),
            name: Set("OmniBridge".to_string()),
            ..Default::default()
        },
    ])
    .exec(db)
    .await
    .unwrap();

    bridge_contracts::Entity::insert_many([
        bridge_contracts::ActiveModel {
            id: Set(1),
            bridge_id: Set(1),
            chain_id: Set(1),
            address: Set(address!("0x4C36d2919e407f0Cc2Ee3c993ccF8ac26d9CE64e").as_slice().to_vec()),
            ..Default::default()
        },
        bridge_contracts::ActiveModel {
            id: Set(2),
            bridge_id: Set(1),
            chain_id: Set(100),
            address: Set(address!("0x75Df5AF045d91108662D8080fD1FEFAd6aA0bb59").as_slice().to_vec()),
            ..Default::default()
        },
    ])
    .exec(db)
    .await
    .unwrap();
}