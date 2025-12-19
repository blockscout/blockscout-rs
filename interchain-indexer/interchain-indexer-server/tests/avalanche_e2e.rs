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

mod helpers;

use anyhow::Result;
use interchain_indexer_entity::{bridges, sea_orm_active_enums::MessageStatus};
use pretty_assertions::assert_eq;
use std::time::Duration;

use alloy::{
    hex,
    network::Ethereum,
    primitives::Address,
    providers::{DynProvider, Provider, ProviderBuilder},
};

/// Helper to convert Option<Vec<u8>> to hex string for readable assertions
fn to_hex(bytes: &Option<Vec<u8>>) -> String {
    bytes
        .as_ref()
        .map(|b| hex::encode_prefixed(b))
        .unwrap_or_else(|| "None".to_string())
}

use interchain_indexer_logic::{
    CrosschainIndexer, InterchainDatabase,
    indexers::avalanche::{AvalancheChainConfig, AvalancheIndexer, AvalancheIndexerConfig},
};
use interchain_indexer_server::{BridgeConfig, BridgeContractConfig, ChainConfig};

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

    let chains = vec![
        ChainConfig {
            chain_id: chain_id_src as i64,
            name: name_src.into(),
            native_id: native_id_src.to_string().into(),
            icon: String::new(),
            rpcs: vec![],
        },
        ChainConfig {
            chain_id: chain_id_dest as i64,
            name: name_dest.into(),
            native_id: native_id_dest.to_string().into(),
            icon: String::new(),
            rpcs: vec![],
        },
    ];

    let bridge_id = 1 as u64;
    let bridge_config = BridgeConfig {
        bridge_id: bridge_id as i32,
        name: "Test Bridge".into(),
        bridge_type: "avalanche_native".into(),
        indexer: String::new(),
        enabled: true,
        contracts: vec![
            BridgeContractConfig {
                chain_id: chain_id_src as i64,
                address: teleporter_address.into(),
                started_at_block: block_number_src as i64,
                version: 1,
                abi: None,
            },
            BridgeContractConfig {
                chain_id: chain_id_dest as i64,
                address: teleporter_address.into(),
                started_at_block: block_number_dest as i64,
                version: 1,
                abi: None,
            },
        ],
        api_url: None,
        ui_url: None,
        docs_url: None,
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

    let bridges = [bridges::ActiveModel::from(bridge_config.clone())].to_vec();
    interchain_db.upsert_bridges(bridges).await?;

    let contract_address: Address = teleporter_address.parse()?;
    let avalanche_chains = vec![
        AvalancheChainConfig {
            chain_id: chain_id_src as i64,
            provider: provider_src,
            contract_address,
            start_block: block_number_src as u64,
        },
        AvalancheChainConfig {
            chain_id: chain_id_dest as i64,
            provider: provider_dest,
            contract_address,
            start_block: block_number_dest as u64,
        },
    ];

    let indexer_config = AvalancheIndexerConfig::new(bridge_config.bridge_id, avalanche_chains);

    let indexer =
        AvalancheIndexer::new(std::sync::Arc::new(interchain_db.clone()), indexer_config)?;
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
        .get_crosschain_messages(None, 100, false, None)
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
    assert_eq!(transfer.src_amount, 21633300000000000000u128.into());
    assert_eq!(transfer.dst_amount, 21633300000000000000u128.into());

    indexer.stop().await;

    Ok(())
}
