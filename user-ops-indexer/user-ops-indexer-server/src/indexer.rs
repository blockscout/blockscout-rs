use crate::settings::Settings;
use ethers::{prelude::Provider, utils::to_checksum};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::time::sleep;
use user_ops_indexer_logic::indexer::{
    common_transport::CommonTransport, settings::IndexerSettings, v06, v07, Indexer, IndexerLogic,
};

pub async fn run(
    settings: Settings,
    db_connection: DatabaseConnection,
) -> Result<(), anyhow::Error> {
    let db_connection = Arc::new(db_connection);

    if settings.indexer.entrypoints.v06 {
        start_indexer_with_retries(
            db_connection.clone(),
            settings.indexer.clone(),
            v06::IndexerV06 {
                entry_point: settings.indexer.entrypoints.v06_entry_point,
            },
        )
        .await?;
    } else {
        tracing::warn!("indexer for v0.6 is disabled in settings");
    }

    if settings.indexer.entrypoints.v07 {
        start_indexer_with_retries(
            db_connection.clone(),
            settings.indexer.clone(),
            v07::IndexerV07 {
                entry_point: settings.indexer.entrypoints.v07_entry_point,
            },
        )
        .await?;
    } else {
        tracing::warn!("indexer for v0.7 is disabled in settings");
    }

    Ok(())
}

async fn start_indexer_with_retries<L: IndexerLogic + Sync + Clone + Send + 'static>(
    db_connection: Arc<DatabaseConnection>,
    settings: IndexerSettings,
    logic: L,
) -> anyhow::Result<()> {
    tracing::info!(
        version = L::version(),
        entry_point = to_checksum(&logic.entry_point(), None),
        "connecting to rpc"
    );

    // If the first connect fails, the function will return an error immediately.
    // All subsequent reconnects are done inside tokio task and will not propagate to above.
    let transport = CommonTransport::new(settings.rpc_url.clone()).await?;
    let client = Provider::new(transport);
    let mut indexer = Indexer::new(
        client,
        db_connection.clone(),
        settings.clone(),
        logic.clone(),
    );

    let delay = settings.restart_delay;

    tokio::spawn(async move {
        loop {
            match indexer.start().await {
                Err(err) => {
                    tracing::error!(
                        error = ?err,
                        version = L::version(),
                        ?delay,
                        "indexer stream ended with error, retrying"
                    );
                }
                Ok(_) => {
                    if !settings.realtime.enabled {
                        tracing::info!(
                            version = L::version(),
                            "indexer stream ended without error, exiting"
                        );
                        return;
                    }
                    tracing::error!(
                        version = L::version(),
                        ?delay,
                        "indexer stream ended unexpectedly, retrying"
                    );
                }
            };

            loop {
                sleep(delay).await;

                tracing::info!(version = L::version(), "re-connecting to rpc");

                let transport = match CommonTransport::new(settings.rpc_url.clone()).await {
                    Ok(transport) => transport,
                    Err(err) => {
                        tracing::error!(
                            error = ?err,
                            version = L::version(),
                            ?delay,
                            "failed to reconnect to the rpc, retrying"
                        );
                        continue;
                    }
                };
                let client = Provider::new(transport);
                indexer = Indexer::new(
                    client,
                    db_connection.clone(),
                    settings.clone(),
                    logic.clone(),
                );
                break;
            }
        }
    });

    Ok(())
}
