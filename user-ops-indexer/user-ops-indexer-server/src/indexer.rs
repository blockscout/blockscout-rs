use crate::settings::Settings;
use ethers::prelude::{Provider, Ws};
use sea_orm::DatabaseConnection;
use user_ops_indexer_logic::indexer::{v06, v07};

pub async fn run(
    settings: Settings,
    db_connection: DatabaseConnection,
) -> Result<(), anyhow::Error> {
    tracing::info!("connecting to rpc");

    let ws_client = Ws::connect_with_reconnects(settings.indexer.rpc_url, 20).await?;
    let client = Provider::new(ws_client);

    if settings.indexer.entrypoints.v06 {
        let indexer = user_ops_indexer_logic::indexer::Indexer::new(client.clone(), &db_connection);

        indexer
            .start::<v06::IndexerV06>(
                settings.indexer.concurrency,
                settings.indexer.realtime.enabled,
                settings.indexer.past_rpc_logs_indexer.get_block_range(),
                settings.indexer.past_db_logs_indexer.get_start_block(),
                settings.indexer.past_db_logs_indexer.get_end_block(),
            )
            .await
            .map_err(|err| {
                tracing::error!("failed to start indexer for v0.6: {err}");
                err
            })?;
    }

    if settings.indexer.entrypoints.v07 {
        let indexer = user_ops_indexer_logic::indexer::Indexer::new(client.clone(), &db_connection);

        indexer
            .start::<v07::IndexerV07>(
                settings.indexer.concurrency,
                settings.indexer.realtime.enabled,
                settings.indexer.past_rpc_logs_indexer.get_block_range(),
                settings.indexer.past_db_logs_indexer.get_start_block(),
                settings.indexer.past_db_logs_indexer.get_end_block(),
            )
            .await
            .map_err(|err| {
                tracing::error!("failed to start indexer for v0.7: {err}");
                err
            })?;
    }

    Ok(())
}
