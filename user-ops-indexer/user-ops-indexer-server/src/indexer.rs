use crate::settings::Settings;
use alloy::providers::ProviderBuilder;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::time::sleep;
use user_ops_indexer_logic::indexer::{settings::IndexerSettings, v06, v07, Indexer, IndexerLogic};

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
        version = L::VERSION,
        entry_point = logic.entry_point().to_string(),
        "connecting to rpc"
    );
    // If the first connect fails, the function will return an error immediately.
    // All subsequent reconnects are done inside tokio task and will not propagate to above.
    let mut client = ProviderBuilder::new().on_builtin(&settings.rpc_url).await?;

    tokio::spawn(async move {
        let delay = settings.restart_delay;

        loop {
            let indexer = Indexer::new(
                client.clone(),
                db_connection.clone(),
                settings.clone(),
                logic.clone(),
            );

            match indexer.start().await {
                Err(err) => {
                    tracing::error!(
                        error = ?err,
                        version = L::VERSION,
                        ?delay,
                        "indexer stream ended with error, retrying"
                    );
                }
                Ok(_) => {
                    if !settings.realtime.enabled {
                        tracing::info!(
                            version = L::VERSION,
                            "indexer stream ended without error, exiting"
                        );
                        return;
                    }
                    tracing::error!(
                        version = L::VERSION,
                        ?delay,
                        "indexer stream ended unexpectedly, retrying"
                    );
                }
            };

            loop {
                sleep(delay).await;

                tracing::info!(version = L::VERSION, "re-connecting to rpc");

                client = match ProviderBuilder::new().on_builtin(&settings.rpc_url).await {
                    Ok(client) => client,
                    Err(err) => {
                        tracing::error!(
                            error = ?err,
                            version = L::VERSION,
                            ?delay,
                            "failed to reconnect to the rpc, retrying"
                        );
                        continue;
                    }
                };
                break;
            }
        }
    });

    Ok(())
}
