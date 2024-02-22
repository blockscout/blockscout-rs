use crate::settings::Settings;
use ethers::prelude::Provider;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use user_ops_indexer_logic::indexer::{common_transport::CommonTransport, v06, v07};

pub async fn run(
    settings: Settings,
    db_connection: DatabaseConnection,
) -> Result<(), anyhow::Error> {
    tracing::info!("connecting to rpc");

    let db_connection = Arc::new(db_connection);

    let transport = CommonTransport::new(settings.indexer.rpc_url.clone()).await?;
    let supports_subscriptions = matches!(transport, CommonTransport::Ws(_));
    let client = Provider::new(transport);

    if settings.indexer.entrypoints.v06 {
        let indexer = user_ops_indexer_logic::indexer::Indexer::new(
            client.clone(),
            db_connection.clone(),
            settings.indexer.clone(),
        );

        tokio::spawn(async move {
            indexer
                .start::<v06::IndexerV06>(supports_subscriptions)
                .await
                .map_err(|err| {
                    tracing::error!("failed to start indexer for v0.6: {err}");
                    err
                })
        });
    } else {
        tracing::warn!("indexer for v0.6 is disabled in settings");
    }

    if settings.indexer.entrypoints.v07 {
        let indexer = user_ops_indexer_logic::indexer::Indexer::new(
            client.clone(),
            db_connection.clone(),
            settings.indexer.clone(),
        );

        tokio::spawn(async move {
            indexer
                .start::<v07::IndexerV07>(supports_subscriptions)
                .await
                .map_err(|err| {
                    tracing::error!("failed to start indexer for v0.7: {err}");
                    err
                })
        });
    } else {
        tracing::warn!("indexer for v0.7 is disabled in settings");
    }

    Ok(())
}
