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

use interchain_indexer_entity::{bridges, crosschain_messages};
use pretty_assertions::assert_eq;
use sea_orm::EntityTrait;

use alloy::{primitives::Address, providers::Provider};
use anyhow::Result;
use std::time::Duration;

use interchain_indexer_logic::{
    InterchainDatabase,
    indexers::avalanche::{AvalancheChainConfig, AvalancheIndexerConfig, spawn_indexer},
};
use interchain_indexer_server::{BridgeConfig, BridgeContractConfig, ChainConfig};

// /// Timeout for waiting for indexer to process events.
// const INDEXER_TIMEOUT: Duration = Duration::from_secs(60);

// /// Poll interval for checking database state.
// const POLL_INTERVAL: Duration = Duration::from_millis(500);

// /// Helper to wait for a condition with timeout.
// async fn wait_for_condition<F, Fut>(condition: F, timeout: Duration) -> Result<()>
// where
//     F: Fn() -> Fut,
//     Fut: std::future::Future<Output = bool>,
// {
//     let start = std::time::Instant::now();
//     loop {
//         if condition().await {
//             return Ok(());
//         }
//         if start.elapsed() > timeout {
//             return Err(anyhow::anyhow!("Timeout waiting for condition"));
//         }
//         tokio::time::sleep(POLL_INTERVAL).await;
//     }
// }

/// Verify that Anvil can fork Avalanche C-Chain and we can fetch the target block.
#[tokio::test]
#[ignore = "requires network access and Anvil binary"]
async fn test_anvil_fork_c_chain_block_accessible() -> Result<()> {
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
    let provider_src = helpers::forked_provider(rpc_url_src, block_number_src);

    let (name_dest, rpc_url_dest, block_number_dest, chain_id_dest, native_id_dest) = (
        "Numine",
        "https://subnets.avax.network/numi/mainnet/rpc",
        269775,
        8021,
        "0xd32cc4660bcf8fa7971589f666fddb5ab22aee7e75dcb30b19829a65d4fb0063",
    );
    let provider_dest = helpers::forked_provider(rpc_url_dest, block_number_dest);

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

    let bridge_config = BridgeConfig {
        bridge_id: 1,
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

    let _indexer_handle = spawn_indexer(interchain_db.clone(), indexer_config)?;

    let start = std::time::Instant::now();
    loop {
        match (
            interchain_db.get_checkpoint(1, chain_id_src).await?,
            interchain_db.get_checkpoint(1, chain_id_dest).await?,
        ) {
            (Some(checkpoint_src), Some(checkpoint_dest))
                if checkpoint_src.realtime_cursor == block_number_src as i64
                    && checkpoint_dest.realtime_cursor == block_number_dest as i64 =>
            {
                break;
            }
            err => {
                dbg!(err);
            }
        };

        if start.elapsed() > Duration::from_secs(5) {
            // return Err(anyhow::anyhow!("timeout"));
            break;
        };

        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Fetch and print all messages
    let all_messages = crosschain_messages::Entity::find().all(db.as_ref()).await?;
    // dbg!(&all_messages);

    assert_eq!(
        interchain_db
            .get_checkpoint(1, chain_id_src)
            .await?
            .unwrap()
            .realtime_cursor,
        block_number_src as i64
    );
    assert_eq!(
        interchain_db
            .get_checkpoint(1, chain_id_dest)
            .await?
            .unwrap()
            .realtime_cursor,
        block_number_dest as i64
    );

    Ok(())
}
