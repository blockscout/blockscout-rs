// SPDX-License-Identifier: LicenseRef-Blockscout

use alloy::primitives::address;
use chrono::{Duration, NaiveDateTime, Utc};
use interchain_indexer_entity::{
    bridge_contracts, bridges, chains, crosschain_messages, crosschain_transfers,
    sea_orm_active_enums::{MessageStatus, TransferType},
};
use sea_orm::{
    ActiveValue::Set,
    DatabaseConnection, EntityTrait,
    prelude::{BigDecimal, Decimal},
};

/// Distinct past timestamps so filter ordering / pagination tests stay stable.
/// Existing rows keep DB `DEFAULT now()` via `..Default::default()`.
fn mock_init_ts(secs_ago: i64) -> NaiveDateTime {
    (Utc::now() - Duration::seconds(secs_ago)).naive_utc()
}

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
        chains::ActiveModel {
            id: Set(250),
            name: Set("Fantom".to_string()),
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
        bridges::ActiveModel {
            id: Set(2),
            name: Set("Teleporter".to_string()),
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
        bridge_contracts::ActiveModel {
            id: Set(3),
            bridge_id: Set(2),
            chain_id: Set(1),
            address: Set(address!("0x00000000000000000000000000000000000000A1")
                .as_slice()
                .to_vec()),
            ..Default::default()
        },
        bridge_contracts::ActiveModel {
            id: Set(4),
            bridge_id: Set(2),
            chain_id: Set(250),
            address: Set(address!("0x00000000000000000000000000000000000000A2")
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

    // Separate insert: SeaORM insert_many cannot mix Set(init_timestamp) with
    // NotSet (DB DEFAULT) in one batch — NotSet becomes NULL and violates NOT NULL.
    crosschain_messages::Entity::insert_many([
        // Bridge 2: 1 → 250
        crosschain_messages::ActiveModel {
            id: Set(1005),
            bridge_id: Set(2),
            status: Set(MessageStatus::Completed),
            init_timestamp: Set(mock_init_ts(30)),
            src_chain_id: Set(1),
            dst_chain_id: Set(Some(250)),
            src_tx_hash: Set(Some(vec![0x33; 32])),
            dst_tx_hash: Set(Some(vec![0x44; 32])),
            sender_address: Set(Some(
                address!("0x00000000000000000000000000000000000000B1")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0x00000000000000000000000000000000000000B2")
                    .as_slice()
                    .to_vec(),
            )),
            payload: Set(Some(vec![1])),
            ..Default::default()
        },
        // Bridge 1: NULL destination (unknown peer)
        crosschain_messages::ActiveModel {
            id: Set(1006),
            bridge_id: Set(1),
            status: Set(MessageStatus::Initiated),
            init_timestamp: Set(mock_init_ts(20)),
            src_chain_id: Set(1),
            dst_chain_id: Set(None),
            src_tx_hash: Set(Some(vec![0x55; 32])),
            dst_tx_hash: Set(None),
            sender_address: Set(Some(
                address!("0x00000000000000000000000000000000000000C1")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(None),
            payload: Set(None),
            ..Default::default()
        },
        // Bridge 1: loopback 100 → 100
        crosschain_messages::ActiveModel {
            id: Set(1007),
            bridge_id: Set(1),
            status: Set(MessageStatus::Completed),
            init_timestamp: Set(mock_init_ts(10)),
            src_chain_id: Set(100),
            dst_chain_id: Set(Some(100)),
            src_tx_hash: Set(Some(vec![0x66; 32])),
            dst_tx_hash: Set(Some(vec![0x77; 32])),
            sender_address: Set(Some(
                address!("0x00000000000000000000000000000000000000D1")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0x00000000000000000000000000000000000000D2")
                    .as_slice()
                    .to_vec(),
            )),
            payload: Set(Some(vec![7])),
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
            index: Set(0),
            r#type: Set(Some(TransferType::Erc20)),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(Some(BigDecimal::from(1_000_000_000_000_000_000u64))), // 1 token with 18 decimals
            dst_amount: Set(Some(BigDecimal::from(1_000_000_000_000_000_000u64))), // 1 token with 18 decimals
            token_src_address: Set(Some(
                address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
                    .as_slice()
                    .to_vec(),
            )),
            token_dst_address: Set(Some(
                address!("0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83")
                    .as_slice()
                    .to_vec(),
            )),
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
            index: Set(0),
            r#type: Set(Some(TransferType::Erc20)),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),

            src_amount: Set(Some(BigDecimal::from(5_000_000_000_000_000_000u64))), // 5 tokens with 18 decimals
            dst_amount: Set(Some(BigDecimal::from(5_000_000_000_000_000_000u64))), // 5 tokens with 18 decimals
            token_src_address: Set(Some(
                address!("0xdAC17F958D2ee523a2206206994597C13D831ec7")
                    .as_slice()
                    .to_vec(),
            )),
            token_dst_address: Set(Some(
                address!("0x4ECaBa5870353805a9F068101A40E0f32ed605C6")
                    .as_slice()
                    .to_vec(),
            )),
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
            index: Set(0),
            r#type: Set(Some(TransferType::Native)),
            token_src_chain_id: Set(100),
            token_dst_chain_id: Set(1),

            src_amount: Set(Some(BigDecimal::from(100_000_000_000_000_000u64))), // 0.1 native token with 18 decimals
            dst_amount: Set(Some(BigDecimal::from(100_000_000_000_000_000u64))), // 0.1 native token with 18 decimals
            token_src_address: Set(Some(vec![0; 20])), // Zero address for native token
            token_dst_address: Set(Some(vec![0; 20])), // Zero address for native token
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
            index: Set(0),
            r#type: Set(Some(TransferType::Erc721)),
            token_src_chain_id: Set(100),
            token_dst_chain_id: Set(1),
            src_amount: Set(Some(BigDecimal::from(1))), // 1 NFT
            dst_amount: Set(Some(BigDecimal::from(1))), // 1 NFT
            token_src_address: Set(Some(
                address!("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")
                    .as_slice()
                    .to_vec(),
            )),
            token_dst_address: Set(Some(
                address!("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599")
                    .as_slice()
                    .to_vec(),
            )),
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
            index: Set(1),
            r#type: Set(Some(TransferType::Erc1155)),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(100),
            src_amount: Set(Some(BigDecimal::from(10u32))), // 10 tokens
            dst_amount: Set(Some(BigDecimal::from(10u32))), // 10 tokens
            token_src_address: Set(Some(
                address!("0x86C80a8aa58e0A4fa09A69624c31Ab2a6CAD56b8")
                    .as_slice()
                    .to_vec(),
            )),
            token_dst_address: Set(Some(
                address!("0x7eB05EfCfE5B672A00f6F4eECeC4d1d75C8f5d2c")
                    .as_slice()
                    .to_vec(),
            )),
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
        // Mirrors message 1005: bridge 2, token 1 → 250
        crosschain_transfers::ActiveModel {
            id: Set(6),
            message_id: Set(1005),
            bridge_id: Set(2),
            index: Set(0),
            r#type: Set(Some(TransferType::Erc20)),
            token_src_chain_id: Set(1),
            token_dst_chain_id: Set(250),
            src_amount: Set(Some(BigDecimal::from(42u32))),
            dst_amount: Set(Some(BigDecimal::from(42u32))),
            token_src_address: Set(Some(
                address!("0x00000000000000000000000000000000000000E1")
                    .as_slice()
                    .to_vec(),
            )),
            token_dst_address: Set(Some(
                address!("0x00000000000000000000000000000000000000E2")
                    .as_slice()
                    .to_vec(),
            )),
            sender_address: Set(Some(
                address!("0x00000000000000000000000000000000000000B1")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0x00000000000000000000000000000000000000B2")
                    .as_slice()
                    .to_vec(),
            )),
            token_ids: Set(None),
            ..Default::default()
        },
        // Mirrors message 1007: loopback token 100 → 100
        crosschain_transfers::ActiveModel {
            id: Set(7),
            message_id: Set(1007),
            bridge_id: Set(1),
            index: Set(0),
            r#type: Set(Some(TransferType::Erc20)),
            token_src_chain_id: Set(100),
            token_dst_chain_id: Set(100),
            src_amount: Set(Some(BigDecimal::from(7u32))),
            dst_amount: Set(Some(BigDecimal::from(7u32))),
            token_src_address: Set(Some(
                address!("0x00000000000000000000000000000000000000F1")
                    .as_slice()
                    .to_vec(),
            )),
            token_dst_address: Set(Some(
                address!("0x00000000000000000000000000000000000000F2")
                    .as_slice()
                    .to_vec(),
            )),
            sender_address: Set(Some(
                address!("0x00000000000000000000000000000000000000D1")
                    .as_slice()
                    .to_vec(),
            )),
            recipient_address: Set(Some(
                address!("0x00000000000000000000000000000000000000D2")
                    .as_slice()
                    .to_vec(),
            )),
            token_ids: Set(None),
            ..Default::default()
        },
    ])
    .exec(db)
    .await
    .unwrap();
}
