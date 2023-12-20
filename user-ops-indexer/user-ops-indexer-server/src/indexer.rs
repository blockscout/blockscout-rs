use crate::settings::Settings;
use ethers::prelude::{Provider, Ws};
use sea_orm::DatabaseConnection;

pub async fn run(
    settings: Settings,
    db_connection: DatabaseConnection,
) -> Result<(), anyhow::Error> {
    tracing::info!("connecting to rpc");

    let ws_client = Ws::connect_with_reconnects(settings.indexer.rpc_url, 3).await?;
    let client = Provider::new(ws_client);

    if settings.indexer.entrypoints.v06 {
        let indexer =
            user_ops_indexer_logic::indexer::v06::indexer::IndexerV06::new(client, &db_connection);

        indexer
            .start(
                settings.indexer.concurrency,
                settings.indexer.realtime.enabled,
                settings.indexer.past_rpc_logs_indexer.get_block_range(),
                settings.indexer.past_db_logs_indexer.get_start_block(),
                settings.indexer.past_db_logs_indexer.get_end_block(),
            )
            .await
            .map_err(|err| {
                tracing::error!("failed to start indexer: {err}");
                err
            })?;
    }

    Ok(())
}
