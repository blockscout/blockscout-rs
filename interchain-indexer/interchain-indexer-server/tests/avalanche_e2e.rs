//! End-to-end tests for Avalanche indexer using Anvil-forked networks.
//!
//! These tests fork live Avalanche networks (C-Chain and NUMINE subnet) at
//! specific blocks containing ICM/ICTT events, then run the indexer to verify
//! correct message processing and database population.
//!
//! ## Test Blocks
//!
//! - **C-Chain block 73334280**: Contains cross-chain message events
//! - **NUMINE block 269775**: Contains cross-chain message events
//!
//! ## Requirements
//!
//! - Anvil binary installed (`foundryup` to install)
//! - Network access to Avalanche RPC endpoints
//! - PostgreSQL database for test database
//!
//! ## Running
//!
//! ```bash
//! cargo test --package interchain-indexer-server avalanche_e2e -- --ignored --nocapture
//! ```

#![cfg(feature = "avalanche-e2e")]

mod helpers;

use anyhow::Result;
use interchain_indexer_entity::{bridges, sea_orm_active_enums::MessageStatus};
use pretty_assertions::assert_eq;
use sea_orm::prelude::BigDecimal;
use std::time::Duration;

use alloy::{
    hex,
    network::Ethereum,
    primitives::Address,
    providers::{DynProvider, Provider, ProviderBuilder},
};

fn decode_blockchain_id(native_id: &str) -> Vec<u8> {
    let native_id = native_id.strip_prefix("0x").unwrap_or(native_id);
    let bytes = hex::decode(native_id).expect("native blockchain id must be hex");
    assert_eq!(bytes.len(), 32, "blockchainID must be 32 bytes");
    bytes
}

/// Helper to convert Option<Vec<u8>> to hex string for readable assertions
fn to_hex(bytes: &Option<Vec<u8>>) -> String {
    bytes
        .as_ref()
        .map(hex::encode_prefixed)
        .unwrap_or_else(|| "None".to_string())
}

fn parse_message_id_from_native_id(native_id: &str) -> i64 {
    let native_id = native_id.strip_prefix("0x").unwrap_or(native_id);
    let bytes = hex::decode(native_id).expect("native id must be hex");
    assert_eq!(bytes.len(), 32, "teleporter messageID must be 32 bytes");
    let first8: [u8; 8] = bytes[..8].try_into().unwrap();
    i64::from_be_bytes(first8)
}

use interchain_indexer_entity::sea_orm_active_enums::BridgeType;
use interchain_indexer_logic::{
    CrosschainIndexer, InterchainDatabase, StatsService,
    indexer::avalanche::{
        AvalancheChainConfig, AvalancheIndexer, settings::AvalancheIndexerSettings,
    },
};
use interchain_indexer_server::{BridgeConfig, BridgeContractConfig, ChainConfig, ExplorerConfig};

/// Create a forked Anvil provider for the given RPC URL and block number.
pub fn forked_provider(rpc_url: &str, block_number: u64) -> DynProvider<Ethereum> {
    ProviderBuilder::new()
        .connect_anvil_with_config(|anvil| anvil.fork_block_number(block_number).fork(rpc_url))
        .erased()
}

/// Verify that Anvil can fork Avalanche C-Chain and we can fetch the target block.
#[tokio::test]
#[ignore = "requires network access and Anvil binary"]
async fn test_icm_and_ictt_are_indexed() -> Result<()> {
    // Initialize tracing for test output
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let (name_src, rpc_url_src, block_number_src, chain_id_src, native_id_src) = (
        "Avalanche C-Chain",
        "https://api.avax.network/ext/bc/C/rpc",
        73334280,
        43114,
        "0x0427d4b22a2a78bcddd456742caf91b56badbff985ee19aef14573e7343fd652",
    );
    let provider_src = forked_provider(rpc_url_src, block_number_src);

    let (name_dest, rpc_url_dest, block_number_dest, chain_id_dest, native_id_dest) = (
        "Numine",
        "https://subnets.avax.network/numi/mainnet/rpc",
        269775,
        8021,
        "0xd32cc4660bcf8fa7971589f666fddb5ab22aee7e75dcb30b19829a65d4fb0063",
    );
    let provider_dest = forked_provider(rpc_url_dest, block_number_dest);

    let teleporter_address = "0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf";

    let chains = [
        ChainConfig {
            chain_id: chain_id_src as i64,
            name: name_src.into(),
            icon: String::new(),
            explorer: ExplorerConfig::default(),
            pool_config: Default::default(),
            rpcs: vec![],
        },
        ChainConfig {
            chain_id: chain_id_dest as i64,
            name: name_dest.into(),
            icon: String::new(),
            explorer: ExplorerConfig::default(),
            pool_config: Default::default(),
            rpcs: vec![],
        },
    ];

    let bridge_id = 1_u64;
    let bridge_config = BridgeConfig {
        bridge_id: bridge_id as i32,
        name: "Test Bridge".into(),
        bridge_type: BridgeType::AvalancheNative,
        indexer_type: Default::default(),
        enabled: true,
        contracts: vec![
            BridgeContractConfig {
                chain_id: chain_id_src as i64,
                address: teleporter_address.into(),
                started_at_block: block_number_src,
                version: 1,
                abi: None,
            },
            BridgeContractConfig {
                chain_id: chain_id_dest as i64,
                address: teleporter_address.into(),
                started_at_block: block_number_dest,
                version: 1,
                abi: None,
            },
        ],
        api_url: None,
        ui_url: None,
        docs_url: None,
        process_unknown_chains: false,
        home_chain_id: None,
    };

    assert_eq!(provider_src.get_block_number().await?, block_number_src);
    assert_eq!(provider_dest.get_block_number().await?, block_number_dest);

    let db_guard = helpers::init_db("avalanche_e2e", "some_test").await;
    let db = db_guard.client();

    let interchain_db = InterchainDatabase::new(db.clone());

    let chains = chains
        .iter()
        .map(|c| interchain_indexer_entity::chains::ActiveModel::from(c.clone()))
        .collect::<Vec<interchain_indexer_entity::chains::ActiveModel>>();
    interchain_db.upsert_chains(chains).await?;

    // Seed blockchainID -> chain_id mapping so the resolver does not need Avalanche Data API.
    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_src),
            chain_id_src as i64,
        )
        .await?;
    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_dest),
            chain_id_dest as i64,
        )
        .await?;

    let bridges = [bridges::ActiveModel::from(bridge_config.clone())].to_vec();
    interchain_db.upsert_bridges(bridges).await?;

    let contract_address: Address = teleporter_address.parse()?;
    let avalanche_chains = vec![
        AvalancheChainConfig {
            chain_id: chain_id_src as i64,
            provider: provider_src,
            contract_address,
            start_block: block_number_src,
        },
        AvalancheChainConfig {
            chain_id: chain_id_dest as i64,
            provider: provider_dest,
            contract_address,
            start_block: block_number_dest,
        },
    ];

    let stats = std::sync::Arc::new(StatsService::new(
        std::sync::Arc::new(interchain_db.clone()),
        None,
        Default::default(),
    ));
    let indexer = AvalancheIndexer::new(
        stats,
        bridge_config.bridge_id,
        avalanche_chains,
        bridge_config.home_chain_id,
        bridge_config.process_unknown_chains,
        &Default::default(),
        &Default::default(),
    )?;
    indexer.start().await?;

    let start = std::time::Instant::now();
    loop {
        if let (Some(checkpoint_src), Some(checkpoint_dest)) = (
            interchain_db
                .get_checkpoint(bridge_id, chain_id_src)
                .await?,
            interchain_db
                .get_checkpoint(bridge_id, chain_id_dest)
                .await?,
        ) && checkpoint_src.realtime_cursor == block_number_src as i64
            && checkpoint_dest.realtime_cursor == block_number_dest as i64
        {
            break;
        };

        if start.elapsed() > Duration::from_secs(5) {
            return Err(anyhow::anyhow!(
                "Timeout waiting for indexer to process blocks"
            ));
        };

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    assert_eq!(
        interchain_db
            .get_checkpoint(bridge_id, chain_id_src)
            .await?
            .unwrap()
            .realtime_cursor,
        block_number_src as i64
    );

    assert_eq!(
        interchain_db
            .get_checkpoint(bridge_id, chain_id_dest)
            .await?
            .unwrap()
            .realtime_cursor,
        block_number_dest as i64
    );

    let (messages, _pagination) = interchain_db
        .get_crosschain_messages(None, None, 100, false, None)
        .await?;

    // Expected message native_id from the test blocks
    let expected_message_native_id =
        "0x6a806e48ef1315a93955b4505ebfbcb9ed45d142bf850c4ce3e67616be485f07";

    let (message, transfers) = messages
        .iter()
        .find(|(m, _)| to_hex(&m.native_id) == expected_message_native_id)
        .expect("Expected message with native_id not found");

    // Verify message fields
    assert_eq!(message.bridge_id, bridge_id as i32);
    assert_eq!(message.src_chain_id, chain_id_src as i64);
    assert_eq!(message.dst_chain_id, Some(chain_id_dest as i64));
    assert_eq!(message.status, MessageStatus::Completed);

    assert_eq!(
        to_hex(&message.src_tx_hash),
        "0xef2cc6c726a60fa322801b9615e7a6f94d4a1388c1f9e7975e03a2af9c781fd8"
    );

    assert_eq!(
        to_hex(&message.dst_tx_hash),
        "0x596767faf86be9f297773c628bded9d3d2b4928ccc6eba852b538afb2a29a52c"
    );

    assert_eq!(
        to_hex(&message.sender_address),
        "0x33a31e0f62c0ddf25090b61ef21a70d5f48725b7"
    );

    assert_eq!(
        to_hex(&message.recipient_address),
        "0x012cb6651cb29c7d5dc96173756a773f7fb87cfb"
    );

    assert!(
        !transfers.is_empty(),
        "Expected at least one transfer for the message"
    );

    let transfer = &transfers[0];
    assert_eq!(transfer.message_id, message.id);
    assert_eq!(transfer.bridge_id, bridge_id as i32);
    // assert_eq!(transfer.r#type, Some(TransferType::Erc20));
    assert_eq!(transfer.token_src_chain_id, chain_id_src as i64);
    assert_eq!(transfer.token_dst_chain_id, chain_id_dest as i64);

    assert_eq!(
        hex::encode_prefixed(&transfer.token_src_address),
        "0x33a31e0f62c0ddf25090b61ef21a70d5f48725b7"
    );

    assert_eq!(
        hex::encode_prefixed(&transfer.token_dst_address),
        "0x012cb6651cb29c7d5dc96173756a773f7fb87cfb"
    );

    assert_eq!(
        to_hex(&transfer.sender_address),
        "0x718245e1a9b44909f89b130e29a8908a9d6bec41"
    );

    assert_eq!(
        to_hex(&transfer.recipient_address),
        "0x718245e1a9b44909f89b130e29a8908a9d6bec41"
    );

    // Verify amounts (sender and recipient should match for a successful transfer)
    assert_eq!(
        transfer.src_amount,
        BigDecimal::from(21633300000000000000u128)
    );
    assert_eq!(
        transfer.dst_amount,
        BigDecimal::from(21633300000000000000u128)
    );

    indexer.stop().await;

    Ok(())
}

/// Verifies that processing only the destination chain (ReceiveCrossChainMessage)
/// does NOT promote messages into `crosschain_messages` without the source-side SendCrossChainMessage.
///
/// Instead, the message should remain pending and be offloaded to `pending_messages` after the hot TTL.
#[tokio::test]
#[ignore = "requires network access and Anvil binary"]
async fn test_receive_only_does_not_promote_message() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let (name_src, rpc_url_src, block_number_src, chain_id_src, native_id_src) = (
        "Avalanche C-Chain",
        "https://api.avax.network/ext/bc/C/rpc",
        73334280,
        43114,
        "0x0427d4b22a2a78bcddd456742caf91b56badbff985ee19aef14573e7343fd652",
    );

    let (name_dest, rpc_url_dest, block_number_dest, chain_id_dest, native_id_dest) = (
        "Numine",
        "https://subnets.avax.network/numi/mainnet/rpc",
        269775,
        8021,
        "0xd32cc4660bcf8fa7971589f666fddb5ab22aee7e75dcb30b19829a65d4fb0063",
    );

    let provider_dest = forked_provider(rpc_url_dest, block_number_dest);
    let quiet_src_block = block_number_src - 1_000;
    let provider_src_quiet = forked_provider(rpc_url_src, quiet_src_block);

    let teleporter_address = "0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf";

    let chains = [
        ChainConfig {
            chain_id: chain_id_src as i64,
            name: name_src.into(),
            icon: String::new(),
            explorer: ExplorerConfig::default(),
            pool_config: Default::default(),
            rpcs: vec![],
        },
        ChainConfig {
            chain_id: chain_id_dest as i64,
            name: name_dest.into(),
            icon: String::new(),
            explorer: ExplorerConfig::default(),
            pool_config: Default::default(),
            rpcs: vec![],
        },
    ];

    let bridge_id = 1u64;
    let bridge_config = BridgeConfig {
        bridge_id: bridge_id as i32,
        name: "Test Bridge".into(),
        bridge_type: BridgeType::AvalancheNative,
        indexer_type: Default::default(),
        enabled: true,
        contracts: vec![
            // Keep both chain contracts in DB so the resolver can resolve sourceBlockchainID.
            BridgeContractConfig {
                chain_id: chain_id_src as i64,
                address: teleporter_address.into(),
                started_at_block: 0,
                version: 1,
                abi: None,
            },
            BridgeContractConfig {
                chain_id: chain_id_dest as i64,
                address: teleporter_address.into(),
                started_at_block: block_number_dest,
                version: 1,
                abi: None,
            },
        ],
        api_url: None,
        ui_url: None,
        docs_url: None,
        process_unknown_chains: false,
        home_chain_id: None,
    };

    assert_eq!(provider_dest.get_block_number().await?, block_number_dest);

    let db_guard = helpers::init_db("avalanche_e2e", "receive_only").await;
    let db = db_guard.client();
    let interchain_db = InterchainDatabase::new(db.clone());

    let chains = chains
        .iter()
        .map(|c| interchain_indexer_entity::chains::ActiveModel::from(c.clone()))
        .collect::<Vec<interchain_indexer_entity::chains::ActiveModel>>();
    interchain_db.upsert_chains(chains).await?;

    // Seed blockchainID -> chain_id mapping so the resolver does not need Avalanche Data API.
    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_src),
            chain_id_src as i64,
        )
        .await?;
    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_dest),
            chain_id_dest as i64,
        )
        .await?;

    let bridges = [bridges::ActiveModel::from(bridge_config.clone())].to_vec();
    interchain_db.upsert_bridges(bridges).await?;

    // Track both chains; source runs on a fork well past the event height so it produces no logs.
    let contract_address: Address = teleporter_address.parse()?;
    let avalanche_chains = vec![
        AvalancheChainConfig {
            chain_id: chain_id_dest as i64,
            provider: provider_dest,
            contract_address,
            start_block: block_number_dest,
        },
        AvalancheChainConfig {
            chain_id: chain_id_src as i64,
            provider: provider_src_quiet,
            contract_address,
            start_block: quiet_src_block,
        },
    ];

    let settings = AvalancheIndexerSettings {
        pull_interval_ms: Duration::from_millis(200),
        batch_size: 25,
        ..Default::default()
    };

    let stats = std::sync::Arc::new(StatsService::new(
        std::sync::Arc::new(interchain_db.clone()),
        None,
        Default::default(),
    ));
    let indexer = AvalancheIndexer::new(
        stats,
        bridge_config.bridge_id,
        avalanche_chains,
        bridge_config.home_chain_id,
        bridge_config.process_unknown_chains,
        &settings,
        &Default::default(),
    )?;
    indexer.start().await?;

    // Expected message native_id from the test blocks (same as full e2e).
    let expected_message_native_id =
        "0x6a806e48ef1315a93955b4505ebfbcb9ed45d142bf850c4ce3e67616be485f07";
    let expected_message_id = parse_message_id_from_native_id(expected_message_native_id);

    // Wait until the receive-only message is offloaded to pending_messages.
    // Hot TTL is ~10s in the default buffer config, so allow a bit more.
    let start = std::time::Instant::now();
    loop {
        let pending = interchain_db
            .get_pending_message(expected_message_id, bridge_id as i32)
            .await?;

        if pending.is_some() {
            break;
        }

        if start.elapsed() > Duration::from_secs(20) {
            return Err(anyhow::anyhow!(
                "Timeout waiting for receive-only message to be offloaded to pending_messages"
            ));
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    // Ensure the message was NOT promoted to final storage.
    let (messages, _pagination) = interchain_db
        .get_crosschain_messages(None, None, 100, false, None)
        .await?;

    assert!(
        messages
            .iter()
            .all(|(m, _)| to_hex(&m.native_id) != expected_message_native_id),
        "Receive-only message must not be promoted to crosschain_messages"
    );

    indexer.stop().await;
    Ok(())
}

/// Verifies that processing only the source chain (SendCrossChainMessage)
/// produces a consistent *initiated* message in `crosschain_messages`.
///
/// We expect:
/// - src_chain_id set (source chain)
/// - dst_chain_id set (resolved destination chain)
/// - status = Initiated
/// - destination-side fields absent (no receive/execution yet)
#[tokio::test]
#[ignore = "requires network access and Anvil binary"]
async fn test_send_only_creates_initiated_message() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let (name_src, rpc_url_src, block_number_src, chain_id_src, native_id_src) = (
        "Avalanche C-Chain",
        "https://api.avax.network/ext/bc/C/rpc",
        73334280,
        43114,
        "0x0427d4b22a2a78bcddd456742caf91b56badbff985ee19aef14573e7343fd652",
    );
    let provider_src = forked_provider(rpc_url_src, block_number_src);

    let (name_dest, rpc_url_dest, block_number_dest, chain_id_dest, native_id_dest) = (
        "Numine",
        "https://subnets.avax.network/numi/mainnet/rpc",
        269775,
        8021,
        "0xd32cc4660bcf8fa7971589f666fddb5ab22aee7e75dcb30b19829a65d4fb0063",
    );

    let teleporter_address = "0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf";

    let chains = [
        ChainConfig {
            chain_id: chain_id_src as i64,
            name: name_src.into(),
            icon: String::new(),
            explorer: ExplorerConfig::default(),
            pool_config: Default::default(),
            rpcs: vec![],
        },
        ChainConfig {
            chain_id: chain_id_dest as i64,
            name: name_dest.into(),
            icon: String::new(),
            explorer: ExplorerConfig::default(),
            pool_config: Default::default(),
            rpcs: vec![],
        },
    ];

    let bridge_id = 1u64;
    let bridge_config = BridgeConfig {
        bridge_id: bridge_id as i32,
        name: "Test Bridge".into(),
        bridge_type: BridgeType::AvalancheNative,
        indexer_type: Default::default(),
        enabled: true,
        contracts: vec![
            // Keep both chain contracts in DB so the resolver can resolve destinationBlockchainID.
            BridgeContractConfig {
                chain_id: chain_id_src as i64,
                address: teleporter_address.into(),
                started_at_block: block_number_src,
                version: 1,
                abi: None,
            },
            BridgeContractConfig {
                chain_id: chain_id_dest as i64,
                address: teleporter_address.into(),
                started_at_block: 0,
                version: 1,
                abi: None,
            },
        ],
        api_url: None,
        ui_url: None,
        docs_url: None,
        process_unknown_chains: false,
        home_chain_id: None,
    };

    assert_eq!(provider_src.get_block_number().await?, block_number_src);

    let db_guard = helpers::init_db("avalanche_e2e", "send_only").await;
    let db = db_guard.client();
    let interchain_db = InterchainDatabase::new(db.clone());

    let chains = chains
        .iter()
        .map(|c| interchain_indexer_entity::chains::ActiveModel::from(c.clone()))
        .collect::<Vec<interchain_indexer_entity::chains::ActiveModel>>();
    interchain_db.upsert_chains(chains).await?;

    // Seed blockchainID -> chain_id mapping so the resolver does not need Avalanche Data API.
    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_src),
            chain_id_src as i64,
        )
        .await?;
    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_dest),
            chain_id_dest as i64,
        )
        .await?;

    let bridges = [bridges::ActiveModel::from(bridge_config.clone())].to_vec();
    interchain_db.upsert_bridges(bridges).await?;
    let quiet_dest_block = block_number_dest - 1_000;
    // Track both chains; destination runs on a fork past the event height so it produces no logs.
    let provider_dest_quiet = forked_provider(rpc_url_dest, quiet_dest_block);
    let contract_address: Address = teleporter_address.parse()?;
    let avalanche_chains = vec![
        AvalancheChainConfig {
            chain_id: chain_id_src as i64,
            provider: provider_src,
            contract_address,
            start_block: block_number_src,
        },
        AvalancheChainConfig {
            chain_id: chain_id_dest as i64,
            provider: provider_dest_quiet,
            contract_address,
            start_block: quiet_dest_block,
        },
    ];

    let settings = AvalancheIndexerSettings {
        pull_interval_ms: Duration::from_millis(200),
        batch_size: 25,
        ..Default::default()
    };

    let stats = std::sync::Arc::new(StatsService::new(
        std::sync::Arc::new(interchain_db.clone()),
        None,
        Default::default(),
    ));
    let indexer = AvalancheIndexer::new(
        stats,
        bridge_config.bridge_id,
        avalanche_chains,
        bridge_config.home_chain_id,
        bridge_config.process_unknown_chains,
        &settings,
        &Default::default(),
    )?;
    indexer.start().await?;

    // Expected message native_id from the test blocks.
    let expected_message_native_id =
        "0x6a806e48ef1315a93955b4505ebfbcb9ed45d142bf850c4ce3e67616be485f07";

    let start = std::time::Instant::now();
    let (message, _transfers) = loop {
        let (messages, _pagination) = interchain_db
            .get_crosschain_messages(None, None, 100, false, None)
            .await?;

        if let Some(found) = messages
            .into_iter()
            .find(|(m, _)| to_hex(&m.native_id) == expected_message_native_id)
        {
            break found;
        }

        if start.elapsed() > Duration::from_secs(8) {
            return Err(anyhow::anyhow!(
                "Timeout waiting for send-only message to be flushed into crosschain_messages"
            ));
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    assert_eq!(message.bridge_id, bridge_id as i32);
    assert_eq!(message.src_chain_id, chain_id_src as i64);
    assert_eq!(message.dst_chain_id, Some(chain_id_dest as i64));
    assert_eq!(message.status, MessageStatus::Initiated);

    // Destination-side fields should be absent.
    assert_eq!(to_hex(&message.dst_tx_hash), "None");
    assert!(
        message.last_update_timestamp.is_none(),
        "last_update_timestamp must be None when only send-side was indexed"
    );

    indexer.stop().await;
    Ok(())
}

/// Verifies that send-side processing still works when the destination chain
/// is not tracked, as long as bridge `home_chain` is set to the source chain.
#[tokio::test]
#[ignore = "requires network access and Anvil binary"]
async fn test_send_only_processes_unknown_destination_when_allowed() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let (name_src, rpc_url_src, block_number_src, chain_id_src, native_id_src) = (
        "Avalanche C-Chain",
        "https://api.avax.network/ext/bc/C/rpc",
        73334280,
        43114,
        "0x0427d4b22a2a78bcddd456742caf91b56badbff985ee19aef14573e7343fd652",
    );
    let chain_id_dest = 8021; // Numine

    let provider_src = forked_provider(rpc_url_src, block_number_src);

    let teleporter_address = "0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf";

    // Only the source chain is tracked by the indexer. Destination is intentionally omitted
    // to exercise bridge-level `home_chain_id` filtering.
    let chains = [ChainConfig {
        chain_id: chain_id_src as i64,
        name: name_src.into(),
        icon: String::new(),
        explorer: ExplorerConfig::default(),
        pool_config: Default::default(),
        rpcs: vec![],
    }];

    let bridge_id = 1u64;
    let bridge_config = BridgeConfig {
        bridge_id: bridge_id as i32,
        name: "Test Bridge".into(),
        bridge_type: BridgeType::AvalancheNative,
        indexer_type: Default::default(),
        enabled: true,
        contracts: vec![
            // Keep both chain contracts in DB so the resolver can resolve destinationBlockchainID.
            BridgeContractConfig {
                chain_id: chain_id_src as i64,
                address: teleporter_address.into(),
                started_at_block: block_number_src,
                version: 1,
                abi: None,
            },
        ],
        api_url: None,
        ui_url: None,
        docs_url: None,
        process_unknown_chains: true,
        home_chain_id: Some(chain_id_src),
    };

    assert_eq!(provider_src.get_block_number().await?, block_number_src);

    let db_guard = helpers::init_db("avalanche_e2e", "send_only_unknown_dest").await;
    let db = db_guard.client();
    let interchain_db = InterchainDatabase::new(db.clone());

    let chains = chains
        .iter()
        .map(|c| interchain_indexer_entity::chains::ActiveModel::from(c.clone()))
        .collect::<Vec<interchain_indexer_entity::chains::ActiveModel>>();
    interchain_db.upsert_chains(chains).await?;

    // Seed blockchainID -> chain_id mapping so the resolver does not need Avalanche Data API.
    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_src),
            chain_id_src as i64,
        )
        .await?;

    let bridges = [bridges::ActiveModel::from(bridge_config.clone())].to_vec();
    interchain_db.upsert_bridges(bridges).await?;

    let contract_address: Address = teleporter_address.parse()?;
    let avalanche_chains = vec![AvalancheChainConfig {
        chain_id: chain_id_src as i64,
        provider: provider_src,
        contract_address,
        start_block: block_number_src,
    }];

    let settings = AvalancheIndexerSettings {
        pull_interval_ms: Duration::from_millis(200),
        batch_size: 25,
        ..Default::default()
    };

    let stats = std::sync::Arc::new(StatsService::new(
        std::sync::Arc::new(interchain_db.clone()),
        None,
        Default::default(),
    ));
    let indexer = AvalancheIndexer::new(
        stats,
        bridge_config.bridge_id,
        avalanche_chains,
        bridge_config.home_chain_id,
        bridge_config.process_unknown_chains,
        &settings,
        &Default::default(),
    )?;
    indexer.start().await?;

    // Expected message native_id from the test blocks.
    let expected_message_native_id =
        "0x6a806e48ef1315a93955b4505ebfbcb9ed45d142bf850c4ce3e67616be485f07";

    let start = std::time::Instant::now();
    let (message, _transfers) = loop {
        let (messages, _pagination) = interchain_db
            .get_crosschain_messages(None, None, 100, false, None)
            .await?;

        if let Some(found) = messages
            .into_iter()
            .find(|(m, _)| to_hex(&m.native_id) == expected_message_native_id)
        {
            break found;
        }

        if start.elapsed() > Duration::from_secs(8) {
            return Err(anyhow::anyhow!(
                "Timeout waiting for send-only message to be flushed into crosschain_messages"
            ));
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    assert_eq!(message.bridge_id, bridge_id as i32);
    assert_eq!(message.src_chain_id, chain_id_src as i64);
    assert_eq!(message.dst_chain_id, Some(chain_id_dest as i64));
    assert_eq!(message.status, MessageStatus::Initiated);

    // Destination-side fields should be absent.
    assert_eq!(to_hex(&message.dst_tx_hash), "None");
    assert!(
        message.last_update_timestamp.is_none(),
        "last_update_timestamp must be None when only send-side was indexed, even with unknown destination"
    );

    // Destination chain is not tracked by the indexer; ensure no checkpoint exists for it.
    assert!(
        interchain_db
            .get_checkpoint(bridge_id, chain_id_dest)
            .await?
            .is_none()
    );

    indexer.stop().await;
    Ok(())
}

/// Verifies that when only the destination chain is configured and
/// bridge `home_chain` set to destination chain,
/// messages from unknown source chains are consolidated using the destination
/// timestamp as `init_timestamp`.
///
/// Expected behavior:
/// - Message appears in `crosschain_messages` (consolidated without send event)
/// - `init_timestamp == last_update_timestamp` (destination-side timestamp used)
/// - `src_tx_hash = None` (source chain is not indexed)
/// - `status = Completed` (execution succeeded on destination)
/// - No ICTT transfer records (source-side TokensSent not available)
#[tokio::test]
#[ignore = "requires network access and Anvil binary"]
async fn test_unknown_source_consolidates_with_destination_timestamp() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    // Source chain is NOT configured - it's "unknown".
    let chain_id_src = 43114;

    let (name_dest, rpc_url_dest, block_number_dest, chain_id_dest, native_id_dest) = (
        "Numine",
        "https://subnets.avax.network/numi/mainnet/rpc",
        269775,
        8021,
        "0xd32cc4660bcf8fa7971589f666fddb5ab22aee7e75dcb30b19829a65d4fb0063",
    );
    let provider_dest = forked_provider(rpc_url_dest, block_number_dest);

    let teleporter_address = "0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf";

    // Only destination chain is configured for the indexer.
    let chains = [ChainConfig {
        chain_id: chain_id_dest as i64,
        name: name_dest.into(),
        icon: String::new(),
        explorer: ExplorerConfig::default(),
        pool_config: Default::default(),
        rpcs: vec![],
    }];

    let bridge_id = 1u64;
    let bridge_config = BridgeConfig {
        bridge_id: bridge_id as i32,
        name: "Test Bridge".into(),
        bridge_type: BridgeType::AvalancheNative,
        indexer_type: Default::default(),
        enabled: true,
        contracts: vec![BridgeContractConfig {
            chain_id: chain_id_dest as i64,
            address: teleporter_address.into(),
            started_at_block: block_number_dest,
            version: 1,
            abi: None,
        }],
        api_url: None,
        ui_url: None,
        docs_url: None,
        process_unknown_chains: true,
        home_chain_id: Some(chain_id_dest),
    };

    assert_eq!(provider_dest.get_block_number().await?, block_number_dest);

    let db_guard = helpers::init_db("avalanche_e2e", "unknown_source_consolidates").await;
    let db = db_guard.client();
    let interchain_db = InterchainDatabase::new(db.clone());

    let chains = chains
        .iter()
        .map(|c| interchain_indexer_entity::chains::ActiveModel::from(c.clone()))
        .collect::<Vec<interchain_indexer_entity::chains::ActiveModel>>();
    interchain_db.upsert_chains(chains).await?;

    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_dest),
            chain_id_dest as i64,
        )
        .await?;

    let bridges = [bridges::ActiveModel::from(bridge_config.clone())].to_vec();
    interchain_db.upsert_bridges(bridges).await?;

    let contract_address: Address = teleporter_address.parse()?;
    // Only destination chain has a provider.
    let avalanche_chains = vec![AvalancheChainConfig {
        chain_id: chain_id_dest as i64,
        provider: provider_dest,
        contract_address,
        start_block: block_number_dest,
    }];

    let settings = AvalancheIndexerSettings {
        pull_interval_ms: Duration::from_millis(200),
        batch_size: 25,
        ..Default::default()
    };

    let stats = std::sync::Arc::new(StatsService::new(
        std::sync::Arc::new(interchain_db.clone()),
        None,
        Default::default(),
    ));
    let indexer = AvalancheIndexer::new(
        stats,
        bridge_config.bridge_id,
        avalanche_chains,
        bridge_config.home_chain_id,
        bridge_config.process_unknown_chains,
        &settings,
        &Default::default(),
    )?;
    indexer.start().await?;

    let expected_message_native_id =
        "0x6a806e48ef1315a93955b4505ebfbcb9ed45d142bf850c4ce3e67616be485f07";

    // Wait for the message to appear in crosschain_messages.
    let start = std::time::Instant::now();
    let (message, transfers) = loop {
        let (messages, _pagination) = interchain_db
            .get_crosschain_messages(None, None, 100, false, None)
            .await?;

        if let Some(found) = messages
            .into_iter()
            .find(|(m, _)| to_hex(&m.native_id) == expected_message_native_id)
        {
            break found;
        }

        if start.elapsed() > Duration::from_secs(15) {
            return Err(anyhow::anyhow!(
                "Timeout waiting for unknown-source message to be consolidated"
            ));
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    // Verify message fields for unknown source chain scenario.
    assert_eq!(message.bridge_id, bridge_id as i32);
    assert_eq!(message.src_chain_id, chain_id_src as i64);
    assert_eq!(message.dst_chain_id, Some(chain_id_dest as i64));
    assert_eq!(
        message.status,
        MessageStatus::Completed,
        "message should be Completed since execution succeeded on destination"
    );

    // init_timestamp should equal last_update_timestamp (both from destination side).
    assert_eq!(
        message.init_timestamp,
        message.last_update_timestamp.unwrap(),
        "init_timestamp should equal last_update_timestamp when source is unknown"
    );

    // No source tx hash since we don't have the send event.
    assert_eq!(
        to_hex(&message.src_tx_hash),
        "None",
        "src_tx_hash should be None when source chain is unknown"
    );

    // Destination tx hash should be present.
    assert_ne!(
        to_hex(&message.dst_tx_hash),
        "None",
        "dst_tx_hash should be present from receive/execution event"
    );

    // No ICTT transfers (source-side TokensSent not available).
    assert!(
        transfers.is_empty(),
        "No ICTT transfers should be present when source chain is unknown (no TokensSent event)"
    );

    indexer.stop().await;
    Ok(())
}

/// Verifies `(process_unknown_chains = true, home_chain_id = None)` behavior.
///
/// With only destination configured, a message from unknown source should still
/// be processed and consolidated because unknown-chain handling is enabled and
/// there is no home-chain narrowing.
#[tokio::test]
#[ignore = "requires network access and Anvil binary"]
async fn test_unknown_source_consolidates_when_allowed_without_home_chain() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    // Source chain is NOT configured - it's "unknown".
    let chain_id_src = 43114;

    let (name_dest, rpc_url_dest, block_number_dest, chain_id_dest, native_id_dest) = (
        "Numine",
        "https://subnets.avax.network/numi/mainnet/rpc",
        269775,
        8021,
        "0xd32cc4660bcf8fa7971589f666fddb5ab22aee7e75dcb30b19829a65d4fb0063",
    );
    let provider_dest = forked_provider(rpc_url_dest, block_number_dest);

    let teleporter_address = "0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf";

    // Only destination chain is configured for the indexer.
    let chains = [ChainConfig {
        chain_id: chain_id_dest as i64,
        name: name_dest.into(),
        icon: String::new(),
        explorer: ExplorerConfig::default(),
        pool_config: Default::default(),
        rpcs: vec![],
    }];

    let bridge_id = 1u64;
    let bridge_config = BridgeConfig {
        bridge_id: bridge_id as i32,
        name: "Test Bridge".into(),
        bridge_type: BridgeType::AvalancheNative,
        indexer_type: Default::default(),
        enabled: true,
        contracts: vec![BridgeContractConfig {
            chain_id: chain_id_dest as i64,
            address: teleporter_address.into(),
            started_at_block: block_number_dest,
            version: 1,
            abi: None,
        }],
        api_url: None,
        ui_url: None,
        docs_url: None,
        process_unknown_chains: true,
        home_chain_id: None,
    };

    assert_eq!(provider_dest.get_block_number().await?, block_number_dest);

    let db_guard = helpers::init_db("avalanche_e2e", "unknown_source_no_home").await;
    let db = db_guard.client();
    let interchain_db = InterchainDatabase::new(db.clone());

    let chains = chains
        .iter()
        .map(|c| interchain_indexer_entity::chains::ActiveModel::from(c.clone()))
        .collect::<Vec<interchain_indexer_entity::chains::ActiveModel>>();
    interchain_db.upsert_chains(chains).await?;

    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_dest),
            chain_id_dest as i64,
        )
        .await?;

    let bridges = [bridges::ActiveModel::from(bridge_config.clone())].to_vec();
    interchain_db.upsert_bridges(bridges).await?;

    let contract_address: Address = teleporter_address.parse()?;
    let avalanche_chains = vec![AvalancheChainConfig {
        chain_id: chain_id_dest as i64,
        provider: provider_dest,
        contract_address,
        start_block: block_number_dest,
    }];

    let settings = AvalancheIndexerSettings {
        pull_interval_ms: Duration::from_millis(200),
        batch_size: 25,
        ..Default::default()
    };

    let stats = std::sync::Arc::new(StatsService::new(
        std::sync::Arc::new(interchain_db.clone()),
        None,
        Default::default(),
    ));
    let indexer = AvalancheIndexer::new(
        stats,
        bridge_config.bridge_id,
        avalanche_chains,
        bridge_config.home_chain_id,
        bridge_config.process_unknown_chains,
        &settings,
        &Default::default(),
    )?;
    indexer.start().await?;

    let expected_message_native_id =
        "0x6a806e48ef1315a93955b4505ebfbcb9ed45d142bf850c4ce3e67616be485f07";

    let start = std::time::Instant::now();
    let (message, _transfers) = loop {
        let (messages, _pagination) = interchain_db
            .get_crosschain_messages(None, None, 100, false, None)
            .await?;

        if let Some(found) = messages
            .into_iter()
            .find(|(m, _)| to_hex(&m.native_id) == expected_message_native_id)
        {
            break found;
        }

        if start.elapsed() > Duration::from_secs(15) {
            return Err(anyhow::anyhow!(
                "Timeout waiting for unknown-source message without home_chain_id"
            ));
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    assert_eq!(message.bridge_id, bridge_id as i32);
    assert_eq!(message.src_chain_id, chain_id_src as i64);
    assert_eq!(message.dst_chain_id, Some(chain_id_dest as i64));
    assert_eq!(message.status, MessageStatus::Completed);

    indexer.stop().await;
    Ok(())
}

/// Verifies `(process_unknown_chains = false, home_chain_id = Some(h))`
/// behavior for unknown-source traffic.
///
/// Even when `home_chain_id` matches the configured destination chain, unknown
/// source messages must be skipped because strict unknown-chain filtering is
/// applied first.
#[tokio::test]
#[ignore = "requires network access and Anvil binary"]
async fn test_home_chain_does_not_override_strict_unknown_filter() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let (chain_id_src, native_id_src) = (
        43114,
        "0x0427d4b22a2a78bcddd456742caf91b56badbff985ee19aef14573e7343fd652",
    );

    let (name_dest, rpc_url_dest, block_number_dest, chain_id_dest, native_id_dest) = (
        "Numine",
        "https://subnets.avax.network/numi/mainnet/rpc",
        269775,
        8021,
        "0xd32cc4660bcf8fa7971589f666fddb5ab22aee7e75dcb30b19829a65d4fb0063",
    );
    let provider_dest = forked_provider(rpc_url_dest, block_number_dest);

    let teleporter_address = "0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf";

    // Only destination chain configured.
    let chains = [ChainConfig {
        chain_id: chain_id_dest as i64,
        name: name_dest.into(),
        icon: String::new(),
        explorer: ExplorerConfig::default(),
        pool_config: Default::default(),
        rpcs: vec![],
    }];

    let bridge_id = 1u64;
    let bridge_config = BridgeConfig {
        bridge_id: bridge_id as i32,
        name: "Test Bridge".into(),
        bridge_type: BridgeType::AvalancheNative,
        indexer_type: Default::default(),
        enabled: true,
        contracts: vec![BridgeContractConfig {
            chain_id: chain_id_dest as i64,
            address: teleporter_address.into(),
            started_at_block: block_number_dest,
            version: 1,
            abi: None,
        }],
        api_url: None,
        ui_url: None,
        docs_url: None,
        process_unknown_chains: false,
        home_chain_id: Some(chain_id_dest),
    };

    assert_eq!(provider_dest.get_block_number().await?, block_number_dest);

    let db_guard = helpers::init_db("avalanche_e2e", "strict_home_unknown_filter").await;
    let db = db_guard.client();
    let interchain_db = InterchainDatabase::new(db.clone());

    let chains = chains
        .iter()
        .map(|c| interchain_indexer_entity::chains::ActiveModel::from(c.clone()))
        .collect::<Vec<interchain_indexer_entity::chains::ActiveModel>>();
    interchain_db.upsert_chains(chains).await?;

    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_dest),
            chain_id_dest as i64,
        )
        .await?;

    let bridges = [bridges::ActiveModel::from(bridge_config.clone())].to_vec();
    interchain_db.upsert_bridges(bridges).await?;

    let contract_address: Address = teleporter_address.parse()?;
    let avalanche_chains = vec![AvalancheChainConfig {
        chain_id: chain_id_dest as i64,
        provider: provider_dest,
        contract_address,
        start_block: block_number_dest,
    }];

    let settings = AvalancheIndexerSettings {
        pull_interval_ms: Duration::from_millis(200),
        batch_size: 25,
        ..Default::default()
    };

    let stats = std::sync::Arc::new(StatsService::new(
        std::sync::Arc::new(interchain_db.clone()),
        None,
        Default::default(),
    ));
    let indexer = AvalancheIndexer::new(
        stats,
        bridge_config.bridge_id,
        avalanche_chains,
        bridge_config.home_chain_id,
        bridge_config.process_unknown_chains,
        &settings,
        &Default::default(),
    )?;
    indexer.start().await?;

    let expected_message_native_id =
        "0x6a806e48ef1315a93955b4505ebfbcb9ed45d142bf850c4ce3e67616be485f07";
    let expected_message_id = parse_message_id_from_native_id(expected_message_native_id);

    // Wait until the unknown source blockchainID has been resolved and persisted.
    // In strict mode the message is skipped, so checkpoints may never be created.
    let start = std::time::Instant::now();
    loop {
        if interchain_db
            .get_avalanche_icm_chain_id_by_blockchain_id(&decode_blockchain_id(native_id_src))
            .await?
            == Some(chain_id_src as i64)
        {
            break;
        }

        if start.elapsed() > Duration::from_secs(8) {
            return Err(anyhow::anyhow!(
                "Timeout waiting for source blockchainID resolution before strict filter assertions"
            ));
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let (messages, _pagination) = interchain_db
        .get_crosschain_messages(None, None, 100, false, None)
        .await?;
    assert!(
        messages
            .iter()
            .all(|(m, _)| to_hex(&m.native_id) != expected_message_native_id),
        "unknown-source message should be filtered out in strict mode even when home_chain_id matches"
    );

    assert!(
        interchain_db
            .get_pending_message(expected_message_id, bridge_id as i32)
            .await?
            .is_none(),
        "filtered message must not enter pending_messages"
    );

    indexer.stop().await;
    Ok(())
}

/// Verifies that when both chains are configured, messages wait for the
/// send event before being consolidated, even if receive/execution arrive first.
///
/// This ensures the `source_chain_is_unknown` flag is NOT set when the source
/// chain is in `chain_ids`, preserving the requirement to wait for the send event.
#[tokio::test]
#[ignore = "requires network access and Anvil binary"]
async fn test_configured_source_waits_for_send() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let (name_src, rpc_url_src, block_number_src, chain_id_src, native_id_src) = (
        "Avalanche C-Chain",
        "https://api.avax.network/ext/bc/C/rpc",
        73334280,
        43114,
        "0x0427d4b22a2a78bcddd456742caf91b56badbff985ee19aef14573e7343fd652",
    );

    let (name_dest, rpc_url_dest, block_number_dest, chain_id_dest, native_id_dest) = (
        "Numine",
        "https://subnets.avax.network/numi/mainnet/rpc",
        269775,
        8021,
        "0xd32cc4660bcf8fa7971589f666fddb5ab22aee7e75dcb30b19829a65d4fb0063",
    );

    let provider_dest = forked_provider(rpc_url_dest, block_number_dest);

    // Source chain is configured but starts at a quiet block (no events).
    let quiet_src_block = block_number_src - 1_000;
    let provider_src_quiet = forked_provider(rpc_url_src, quiet_src_block);

    let teleporter_address = "0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf";

    let chains = [
        ChainConfig {
            chain_id: chain_id_src as i64,
            name: name_src.into(),
            icon: String::new(),
            explorer: ExplorerConfig::default(),
            pool_config: Default::default(),
            rpcs: vec![],
        },
        ChainConfig {
            chain_id: chain_id_dest as i64,
            name: name_dest.into(),
            icon: String::new(),
            explorer: ExplorerConfig::default(),
            pool_config: Default::default(),
            rpcs: vec![],
        },
    ];

    let bridge_id = 1u64;
    let bridge_config = BridgeConfig {
        bridge_id: bridge_id as i32,
        name: "Test Bridge".into(),
        bridge_type: BridgeType::AvalancheNative,
        indexer_type: Default::default(),
        enabled: true,
        contracts: vec![
            BridgeContractConfig {
                chain_id: chain_id_src as i64,
                address: teleporter_address.into(),
                started_at_block: 0,
                version: 1,
                abi: None,
            },
            BridgeContractConfig {
                chain_id: chain_id_dest as i64,
                address: teleporter_address.into(),
                started_at_block: block_number_dest,
                version: 1,
                abi: None,
            },
        ],
        api_url: None,
        ui_url: None,
        docs_url: None,
        process_unknown_chains: false,
        home_chain_id: None,
    };

    let db_guard = helpers::init_db("avalanche_e2e", "configured_source_waits").await;
    let db = db_guard.client();
    let interchain_db = InterchainDatabase::new(db.clone());

    let chains = chains
        .iter()
        .map(|c| interchain_indexer_entity::chains::ActiveModel::from(c.clone()))
        .collect::<Vec<interchain_indexer_entity::chains::ActiveModel>>();
    interchain_db.upsert_chains(chains).await?;

    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_src),
            chain_id_src as i64,
        )
        .await?;
    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_dest),
            chain_id_dest as i64,
        )
        .await?;

    let bridges = [bridges::ActiveModel::from(bridge_config.clone())].to_vec();
    interchain_db.upsert_bridges(bridges).await?;

    let contract_address: Address = teleporter_address.parse()?;

    // BOTH chains are tracked. Source starts at a quiet block to simulate lag.
    let avalanche_chains = vec![
        AvalancheChainConfig {
            chain_id: chain_id_dest as i64,
            provider: provider_dest,
            contract_address,
            start_block: block_number_dest,
        },
        AvalancheChainConfig {
            chain_id: chain_id_src as i64,
            provider: provider_src_quiet,
            contract_address,
            start_block: quiet_src_block,
        },
    ];

    let settings = AvalancheIndexerSettings {
        pull_interval_ms: Duration::from_millis(200),
        batch_size: 25,
        ..Default::default()
    };

    let stats = std::sync::Arc::new(StatsService::new(
        std::sync::Arc::new(interchain_db.clone()),
        None,
        Default::default(),
    ));
    let indexer = AvalancheIndexer::new(
        stats,
        bridge_config.bridge_id,
        avalanche_chains,
        bridge_config.home_chain_id,
        bridge_config.process_unknown_chains,
        &settings,
        &Default::default(),
    )?;
    indexer.start().await?;

    let expected_message_native_id =
        "0x6a806e48ef1315a93955b4505ebfbcb9ed45d142bf850c4ce3e67616be485f07";
    let expected_message_id = parse_message_id_from_native_id(expected_message_native_id);

    // Wait for destination chain to process the receive event.
    // The message should be in pending_messages (buffer offloaded) but NOT in crosschain_messages.
    let start = std::time::Instant::now();
    loop {
        let pending = interchain_db
            .get_pending_message(expected_message_id, bridge_id as i32)
            .await?;

        if pending.is_some() {
            break;
        }

        if start.elapsed() > Duration::from_secs(20) {
            return Err(anyhow::anyhow!(
                "Timeout waiting for receive-only message to be offloaded to pending_messages"
            ));
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    // Verify message is NOT consolidated into crosschain_messages yet.
    let (messages, _pagination) = interchain_db
        .get_crosschain_messages(None, None, 100, false, None)
        .await?;

    assert!(
        messages
            .iter()
            .all(|(m, _)| to_hex(&m.native_id) != expected_message_native_id),
        "Message from configured source must NOT be consolidated without send event"
    );

    indexer.stop().await;
    Ok(())
}

/// Verifies that `home_chain_id` filtering works for unknown source chain
/// messages.
///
/// When `home_chain_id = Some(dest)`, only messages where one endpoint is the
/// destination chain should be processed when the source is unknown.
#[tokio::test]
#[ignore = "requires network access and Anvil binary"]
async fn test_home_chain_filters_unknown_source() -> Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();

    let chain_id_src = 43114;

    let (name_dest, rpc_url_dest, block_number_dest, chain_id_dest, native_id_dest) = (
        "Numine",
        "https://subnets.avax.network/numi/mainnet/rpc",
        269775,
        8021,
        "0xd32cc4660bcf8fa7971589f666fddb5ab22aee7e75dcb30b19829a65d4fb0063",
    );
    let provider_dest = forked_provider(rpc_url_dest, block_number_dest);

    let teleporter_address = "0x253b2784c75e510dD0fF1da844684a1aC0aa5fcf";

    // Only destination chain configured.
    let chains = [ChainConfig {
        chain_id: chain_id_dest as i64,
        name: name_dest.into(),
        icon: String::new(),
        explorer: ExplorerConfig::default(),
        pool_config: Default::default(),
        rpcs: vec![],
    }];

    let bridge_id = 1u64;
    let bridge_config = BridgeConfig {
        bridge_id: bridge_id as i32,
        name: "Test Bridge".into(),
        bridge_type: BridgeType::AvalancheNative,
        indexer_type: Default::default(),
        enabled: true,
        contracts: vec![BridgeContractConfig {
            chain_id: chain_id_dest as i64,
            address: teleporter_address.into(),
            started_at_block: block_number_dest,
            version: 1,
            abi: None,
        }],
        api_url: None,
        ui_url: None,
        docs_url: None,
        process_unknown_chains: true,
        home_chain_id: Some(chain_id_dest),
    };

    assert_eq!(provider_dest.get_block_number().await?, block_number_dest);

    let db_guard = helpers::init_db("avalanche_e2e", "home_chain_id_filter").await;
    let db = db_guard.client();
    let interchain_db = InterchainDatabase::new(db.clone());

    let chains = chains
        .iter()
        .map(|c| interchain_indexer_entity::chains::ActiveModel::from(c.clone()))
        .collect::<Vec<interchain_indexer_entity::chains::ActiveModel>>();
    interchain_db.upsert_chains(chains).await?;

    interchain_db
        .upsert_avalanche_icm_blockchain_id(
            decode_blockchain_id(native_id_dest),
            chain_id_dest as i64,
        )
        .await?;

    let bridges = [bridges::ActiveModel::from(bridge_config.clone())].to_vec();
    interchain_db.upsert_bridges(bridges).await?;

    let contract_address: Address = teleporter_address.parse()?;
    let avalanche_chains = vec![AvalancheChainConfig {
        chain_id: chain_id_dest as i64,
        provider: provider_dest,
        contract_address,
        start_block: block_number_dest,
    }];

    // home_chain_id = dest chain (Numine, 8021). Unknown source → Numine should be processed.
    let settings = AvalancheIndexerSettings {
        pull_interval_ms: Duration::from_millis(200),
        batch_size: 25,
        ..Default::default()
    };

    let stats = std::sync::Arc::new(StatsService::new(
        std::sync::Arc::new(interchain_db.clone()),
        None,
        Default::default(),
    ));
    let indexer = AvalancheIndexer::new(
        stats,
        bridge_config.bridge_id,
        avalanche_chains,
        bridge_config.home_chain_id,
        bridge_config.process_unknown_chains,
        &settings,
        &Default::default(),
    )?;
    indexer.start().await?;

    let expected_message_native_id =
        "0x6a806e48ef1315a93955b4505ebfbcb9ed45d142bf850c4ce3e67616be485f07";

    // Wait for message to be consolidated.
    let start = std::time::Instant::now();
    let (message, _transfers) = loop {
        let (messages, _pagination) = interchain_db
            .get_crosschain_messages(None, None, 100, false, None)
            .await?;

        if let Some(found) = messages
            .into_iter()
            .find(|(m, _)| to_hex(&m.native_id) == expected_message_native_id)
        {
            break found;
        }

        if start.elapsed() > Duration::from_secs(15) {
            return Err(anyhow::anyhow!(
                "Timeout waiting for home-chain-filtered message to be consolidated"
            ));
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    };

    // Message from unknown source → primary destination should be processed.
    assert_eq!(message.src_chain_id, chain_id_src as i64);
    assert_eq!(message.dst_chain_id, Some(chain_id_dest as i64));
    assert_eq!(
        message.status,
        MessageStatus::Completed,
        "message to primary chain should be consolidated and completed"
    );

    indexer.stop().await;
    Ok(())
}
