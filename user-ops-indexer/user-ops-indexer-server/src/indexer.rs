use crate::settings::Settings;
use alloy::{
    providers::{ProviderBuilder, RootProvider, WsConnect},
    transports::{BoxTransport, TransportError, TransportErrorKind},
};
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
        version = L::version(),
        entry_point = logic.entry_point().to_string(),
        "connecting to rpc"
    );

    // If the first connect fails, the function will return an error immediately.
    // All subsequent reconnects are done inside tokio task and will not propagate to above.
    let mut indexer = Indexer::new(
        connect(settings.rpc_url.clone()).await?,
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

                let provider = match connect(settings.rpc_url.clone()).await {
                    Ok(provider) => provider,
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
                indexer = Indexer::new(
                    provider,
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

async fn connect(rpc_url: String) -> Result<RootProvider<BoxTransport>, TransportError> {
    if rpc_url.starts_with("ws") {
        let ws = WsConnect::new(rpc_url);
        Ok(ProviderBuilder::new().on_ws(ws).await?.boxed())
    } else {
        let http = rpc_url
            .parse()
            .map_err(|_| TransportErrorKind::custom_str("invalid http url"))?;
        Ok(ProviderBuilder::new().on_http(http).boxed())
    }
}
