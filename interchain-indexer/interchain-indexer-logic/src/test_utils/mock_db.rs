use alloy::primitives::address;
use interchain_indexer_entity::{
    bridge_contracts, bridges, chains, crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{MessageStatus, TransferType},
};
use sea_orm::{
    ActiveValue::Set,
    DatabaseConnection, EntityTrait,
    prelude::{BigDecimal, Decimal},
};

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

    bridges::Entity::insert_many([bridges::ActiveModel {
        id: Set(1),
        name: Set("OmniBridge".to_string()),
        ..Default::default()
    }])
    .exec(db)
    .await
    .unwrap();

    bridge_contracts::Entity::insert_many([
        bridge_contracts::ActiveModel {
            id: Set(1),
            bridge_id: Set(1),
            chain_id: Set(1),
            address: Set(address!("0x4C36d2919e407f0Cc2Ee3c993ccF8ac26d9CE64e")
                .as_slice()
                .to_vec()),
            ..Default::default()
        },
        bridge_contracts::ActiveModel {
            id: Set(2),
            bridge_id: Set(1),
            chain_id: Set(100),
            address: Set(address!("0x75Df5AF045d91108662D8080fD1FEFAd6aA0bb59")
                .as_slice()
                .to_vec()),
            ..Default::default()
        },
    ])
    .exec(db)
    .await
    .unwrap();

    crosschain_messages::Entity::insert_many([
        crosschain_messages::ActiveModel {
            id: Set(1001),
            bridge_id: Set(1),
            status: Set(MessageStatus::Initiated),
            src_chain_id: Set(1),
            dst_chain_id: Set(Some(100)),
            src_tx_hash: Set(Some(vec![
                0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab,
                0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78,
                0x90, 0xab, 0xcd, 0xef,
            ])),
            dst_tx_hash: Set(None),
            sender_address: Set(Some(
                address!("0x0000000000000000000000000000000000000001")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0x8ba1f109551bD432803012645ac136c22C929B00")
                    .as_slice()
                    .to_vec(),
            )),
            payload: Set(Some(vec![1, 2, 3, 4, 5])),
            ..Default::default()
        },
        crosschain_messages::ActiveModel {
            id: Set(1002),
            bridge_id: Set(1),
            status: Set(MessageStatus::Completed),
            src_chain_id: Set(1),
            dst_chain_id: Set(Some(100)),
            src_tx_hash: Set(Some(vec![
                0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56,
                0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12,
                0x34, 0x56, 0x78, 0x90,
            ])),
            dst_tx_hash: Set(Some(vec![
                0xfe, 0xdc, 0xba, 0x09, 0x87, 0x65, 0x43, 0x21, 0xfe, 0xdc, 0xba, 0x09, 0x87, 0x65,
                0x43, 0x21, 0xfe, 0xdc, 0xba, 0x09, 0x87, 0x65, 0x43, 0x21, 0xfe, 0xdc, 0xba, 0x09,
                0x87, 0x65, 0x43, 0x21,
            ])),
            sender_address: Set(Some(
                address!("0x9f8F72AA9304c8B593d555F12eF6589cC3A579A2")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
                    .as_slice()
                    .to_vec(),
            )),
            payload: Set(Some(vec![10, 20, 30, 40, 50])),
            ..Default::default()
        },
        crosschain_messages::ActiveModel {
            id: Set(1003),
            bridge_id: Set(1),
            status: Set(MessageStatus::Failed),
            src_chain_id: Set(100),
            dst_chain_id: Set(Some(1)),
            src_tx_hash: Set(Some(vec![0x11; 32])),
            dst_tx_hash: Set(None),
            sender_address: Set(Some(
                address!("0x6B175474E89094C44Da98b954EedeAC495271d0F")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
                    .as_slice()
                    .to_vec(),
            )),
            payload: Set(None),
            ..Default::default()
        },
        crosschain_messages::ActiveModel {
            id: Set(1004),
            bridge_id: Set(1),
            status: Set(MessageStatus::Initiated),
            src_chain_id: Set(100),
            dst_chain_id: Set(Some(1)),
            src_tx_hash: Set(Some(vec![0x22; 32])),
            dst_tx_hash: Set(None),
            sender_address: Set(Some(
                address!("0xdAC17F958D2ee523a2206206994597C13D831ec7")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")
                    .as_slice()
                    .to_vec(),
            )),
            payload: Set(Some(vec![100, 200, 255])),
            ..Default::default()
        },
    ])
    .exec(db)
    .await
    .unwrap();

    crosschain_transfers::Entity::insert_many([
        crosschain_transfers::ActiveModel {
            id: Set(1),
            message_id: Set(1001),
            bridge_id: Set(1),
            r#type: Set(Some(TransferType::Erc20)),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_decimals: Set(18),
            dst_decimals: Set(18),
            src_amount: Set(BigDecimal::from(1_000_000_000_000_000_000u64)), // 1 token with 18 decimals
            dst_amount: Set(BigDecimal::from(1_000_000_000_000_000_000u64)), // 1 token with 18 decimals
            token_src_address: Set(address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
                .as_slice()
                .to_vec()),
            token_dst_address: Set(address!("0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83")
                .as_slice()
                .to_vec()),
            sender_address: Set(Some(
                address!("0x0000000000000000000000000000000000000001")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0x8ba1f109551bD432803012645ac136c22C929B00")
                    .as_slice()
                    .to_vec(),
            )),
            token_ids: Set(None),
            ..Default::default()
        },
        crosschain_transfers::ActiveModel {
            id: Set(2),
            message_id: Set(1002),
            bridge_id: Set(1),
            r#type: Set(Some(TransferType::Erc20)),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_decimals: Set(18),
            dst_decimals: Set(18),
            src_amount: Set(BigDecimal::from(5_000_000_000_000_000_000u64)), // 5 tokens with 18 decimals
            dst_amount: Set(BigDecimal::from(5_000_000_000_000_000_000u64)), // 5 tokens with 18 decimals
            token_src_address: Set(address!("0xdAC17F958D2ee523a2206206994597C13D831ec7")
                .as_slice()
                .to_vec()),
            token_dst_address: Set(address!("0x4ECaBa5870353805a9F068101A40E0f32ed605C6")
                .as_slice()
                .to_vec()),
            sender_address: Set(Some(
                address!("0x9f8F72AA9304c8B593d555F12eF6589cC3A579A2")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
                    .as_slice()
                    .to_vec(),
            )),
            token_ids: Set(None),
            ..Default::default()
        },
        crosschain_transfers::ActiveModel {
            id: Set(3),
            message_id: Set(1003),
            bridge_id: Set(1),
            r#type: Set(Some(TransferType::Native)),
            token_src_chain_id: Set(100),
            token_dst_chain_id: Set(1),
            src_decimals: Set(18),
            dst_decimals: Set(18),
            src_amount: Set(BigDecimal::from(100_000_000_000_000_000u64)), // 0.1 native token with 18 decimals
            dst_amount: Set(BigDecimal::from(100_000_000_000_000_000u64)), // 0.1 native token with 18 decimals
            token_src_address: Set(vec![0; 20]), // Zero address for native token
            token_dst_address: Set(vec![0; 20]), // Zero address for native token
            sender_address: Set(Some(
                address!("0x6B175474E89094C44Da98b954EedeAC495271d0F")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
                    .as_slice()
                    .to_vec(),
            )),
            token_ids: Set(None),
            ..Default::default()
        },
        crosschain_transfers::ActiveModel {
            id: Set(4),
            message_id: Set(1004),
            bridge_id: Set(1),
            r#type: Set(Some(TransferType::Erc721)),
            token_src_chain_id: Set(100),
            token_dst_chain_id: Set(1),
            src_decimals: Set(0),
            dst_decimals: Set(0),
            src_amount: Set(BigDecimal::from(1)), // 1 NFT
            dst_amount: Set(BigDecimal::from(1)), // 1 NFT
            token_src_address: Set(address!("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")
                .as_slice()
                .to_vec()),
            token_dst_address: Set(address!("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")
                .as_slice()
                .to_vec()),
            sender_address: Set(Some(
                address!("0xdAC17F958D2ee523a2206206994597C13D831ec7")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")
                    .as_slice()
                    .to_vec(),
            )),
            token_ids: Set(Some(vec![Decimal::new(12345, 0)])), // NFT token ID
            ..Default::default()
        },
        crosschain_transfers::ActiveModel {
            id: Set(5),
            message_id: Set(1002),
            bridge_id: Set(1),
            r#type: Set(Some(TransferType::Erc1155)),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_decimals: Set(0),
            dst_decimals: Set(0),
            src_amount: Set(BigDecimal::from(10u32)), // 10 tokens
            dst_amount: Set(BigDecimal::from(10u32)), // 10 tokens
            token_src_address: Set(address!("0x86C80a8aa58e0A4fa09A69624c31Ab2a6CAD56b8")
                .as_slice()
                .to_vec()),
            token_dst_address: Set(address!("0x7eB05EfCfE5B672A00f6F4eECeC4d1d75C8f5d2c")
                .as_slice()
                .to_vec()),
            sender_address: Set(Some(
                address!("0x9f8F72AA9304c8B593d555F12eF6589cC3A579A2")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
                    .as_slice()
                    .to_vec(),
            )),
            token_ids: Set(Some(vec![
                Decimal::new(1, 0),
                Decimal::new(2, 0),
                Decimal::new(3, 0),
            ])), // Multiple token IDs
            ..Default::default()
        },
    ])
    .exec(db)
    .await
    .unwrap();
}
