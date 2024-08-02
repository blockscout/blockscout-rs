use da_indexer_logic::indexer::Indexer;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::time::sleep;

use crate::Settings;

pub async fn run(
    settings: Settings,
    db_connection: DatabaseConnection,
) -> Result<(), anyhow::Error> {
    let db_connection = Arc::new(db_connection);

    // If the first connect fails, the function will return an error immediately.
    // All subsequent reconnects are done inside tokio task and will not propagate to above.
    let mut indexer = Indexer::new(db_connection.clone(), settings.indexer.clone()).await?;
    let delay = settings.indexer.restart_delay;

    tokio::spawn(async move {
        loop {
            match indexer.start().await {
                Err(err) => {
                    tracing::error!(error = ?err, ?delay, "indexer startup failed, retrying");
                }
                Ok(_) => {
                    tracing::error!(?delay, "indexer stream ended unexpectedly, retrying");
                }
            };

            loop {
                sleep(delay).await;

                tracing::info!("re-connecting to rpc");

                match Indexer::new(db_connection.clone(), settings.indexer.clone()).await {
                    Ok(new_indexer) => {
                        indexer = new_indexer;
                        break;
                    }
                    Err(err) => {
                        tracing::error!(error = ?err, ?delay, "indexer re-connect failed, retrying");
                    }
                }
            }
        }
    });

    Ok(())
}
