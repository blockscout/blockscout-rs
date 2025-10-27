use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Ok;
use chrono::NaiveDateTime;
use futures::future::join;

use crate::{
    channel::{CCTX_STATUS_UPDATE_TOPIC, NEW_CCTXS_TOPIC},
    client::Client,
    database::ZetachainCctxDatabase,
    models::{CctxShort, PagedCCTXResponse},
    settings::IndexerSettings,
};
use actix_phoenix_channel::{ChannelBroadcaster, ChannelEvent};
use base64::Engine;
use futures::{
    stream::{select_with_strategy, PollNext},
    StreamExt,
};
use tokio::task::JoinHandle;
use tracing::instrument;
use uuid::Uuid;
use zetachain_cctx_entity::sea_orm_active_enums::{Kind, ProcessingStatus};
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::CctxListItem as CctxListItemProto;

fn broadcast_new_cctxs_if_any(
    channel_broadcaster: &ChannelBroadcaster,
    inserted: &Vec<CctxListItemProto>,
) {
    if !inserted.is_empty() {
        channel_broadcaster.broadcast(ChannelEvent::new(NEW_CCTXS_TOPIC, "new_cctxs", inserted));
    }
}
pub struct Indexer {
    pub settings: IndexerSettings,
    pub client: Arc<Client>,
    pub database: Arc<ZetachainCctxDatabase>,
    pub channel_broadcaster: Arc<ChannelBroadcaster>,
}

enum IndexerJob {
    StatusUpdate(CctxShort, Uuid),        //cctx index and id to be updated
    LevelDataGap(i32, String, i32, Uuid), // Watermark (pointer) to the next page of cctxs to be fetched
    HistoricalDataFetch(Uuid, HistoricalDataFetchJob), // Watermark (pointer) to the next page of cctxs to be fetched
    TokenSync(Uuid),                                   // Token synchronization job
}

#[derive(Clone)]
struct HistoricalDataFetchJob {
    watermark_id: i32,
    watermark_pointer: String,
    watermark_upper_bound_timestamp: Option<NaiveDateTime>,
}

#[instrument(,level="debug",skip_all)]
async fn refresh_cctx_status(
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    cctx: &CctxShort,
) -> anyhow::Result<CctxListItemProto> {
    let fetched_cctx = client
        .fetch_cctx(&cctx.index)
        .await
        .map_err(|e| anyhow::anyhow!(format!("Failed to fetch cctx: {}", e)))?;
    let updated = database
        .update_cctx_status(cctx, fetched_cctx)
        .await
        .map_err(|e| anyhow::anyhow!(format!("Failed to update cctx status: {}", e)))?;
    Ok(updated)
}

#[instrument(,level="debug",skip_all, fields(job_id = %job_id, watermark_id = %watermark_id, watermark_pointer = %watermark_pointer))]
async fn level_data_gap(
    job_id: Uuid,
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    watermark_id: i32,
    watermark_pointer: String,
    batch_size: u32,
    realtime_threshold: i64,
) -> anyhow::Result<(ProcessingStatus, Vec<CctxListItemProto>)> {
    let PagedCCTXResponse {
        cross_chain_tx: cctxs,
        pagination,
    } = client
        .list_cctxs(Some(&watermark_pointer), false, batch_size)
        .await?;

    if cctxs.is_empty() {
        tracing::debug!("No recent cctxs found");
        return Ok((ProcessingStatus::Done, Vec::new()));
    }

    let next_key = pagination
        .next_key
        .ok_or(anyhow::anyhow!("next_key is None"))?;
    database
        .import_cctxs_and_move_watermark(job_id, cctxs, &next_key, realtime_threshold, watermark_id)
        .await
}

#[instrument(,level="debug",skip_all, fields(job_id = %job_id))]
async fn historical_sync(
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    job_id: Uuid,
    job: HistoricalDataFetchJob,
    batch_size: u32,
) -> anyhow::Result<()> {
    let response = client
        .list_cctxs(Some(&job.watermark_pointer), true, batch_size)
        .await?;
    let cross_chain_txs = response.cross_chain_tx;
    if cross_chain_txs.is_empty() {
        return Err(anyhow::anyhow!("Node returned empty response"));
    }
    let next_key = response.pagination.next_key.unwrap_or(
        //base64 of the index of the last cctx in the response
        base64::engine::general_purpose::STANDARD
            .encode(cross_chain_txs.last().unwrap().index.as_bytes()),
    );
    //atomically insert cctxs and update watermark
    let imported = database
        .import_cctxs(
            job_id,
            cross_chain_txs,
            &next_key,
            job.watermark_id,
            job.watermark_upper_bound_timestamp,
        )
        .await?;

    tracing::debug!(
        "imported cctxs: {:?}",
        imported
            .iter()
            .map(|c| c.index.clone())
            .collect::<Vec<String>>()
    );
    Ok(())
}

#[instrument(,level="debug",skip(database, client), fields(job_id = %job_id))]
async fn realtime_fetch(
    job_id: Uuid,
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    batch_size: u32,
) -> anyhow::Result<Vec<CctxListItemProto>> {
    let response = client
        .list_cctxs(None, false, batch_size)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch external data: {}", e))?;
    let txs = response.cross_chain_tx;
    if txs.is_empty() {
        tracing::debug!("No new cctxs found");
        return Ok(Vec::new());
    }
    let next_key = response
        .pagination
        .next_key
        .ok_or(anyhow::anyhow!("next_key is None"))?;

    database
        .process_realtime_cctxs(job_id, txs, &next_key)
        .await
}

#[instrument(,level="debug",skip(database, client,cctx), fields(job_id = %job_id, cctx_index = %cctx.index, cctx_id = %cctx.id))]
async fn refresh_status_and_link_related(
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    cctx: &CctxShort,
    job_id: Uuid,
    channel_broadcaster: &ChannelBroadcaster,
) -> anyhow::Result<()> {
    let updated = refresh_cctx_status(database.clone(), client, cctx)
        .await
        .map_err(|e| anyhow::format_err!("Failed to refresh cctx status: {}", e))?;
    channel_broadcaster.broadcast(ChannelEvent::new(
        CCTX_STATUS_UPDATE_TOPIC,
        updated.index.clone(),
        &updated,
    ));

    let inserted = update_cctx_relations(database.clone(), client, cctx, job_id)
        .await
        .map_err(|e| anyhow::format_err!("Failed to update cctx relations: {}", e))?;
    if !inserted.is_empty() {
        channel_broadcaster.broadcast(ChannelEvent::new(NEW_CCTXS_TOPIC, "new_cctxs", &inserted));
    }
    Ok(())
}

#[instrument(,level="debug",skip_all)]
async fn update_cctx_relations(
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    cctx: &CctxShort,
    job_id: Uuid,
) -> anyhow::Result<Vec<CctxListItemProto>> {
    // Fetch children using the inbound hash to CCTX data endpoint
    let cross_chain_txs = client
        .get_inbound_hash_to_cctx_data(&cctx.index)
        .await
        .map_err(|e| anyhow::format_err!("Failed to fetch children: {}", e))?
        .cross_chain_txs;

    //turns out that we migh get duplicate cross_chain_txs, so we deduplicate them
    let cctx_map: HashMap<String, _> = cross_chain_txs
        .into_iter()
        .filter(|c| c.index != cctx.index)
        .map(|c| (c.index.clone(), c))
        .collect();
    let cross_chain_txs = cctx_map.values().cloned().collect::<Vec<_>>();

    let inserted = database
        .traverse_and_update_tree_relationships(cross_chain_txs.clone(), cctx, job_id)
        .await
        .map_err(|e| {
            anyhow::format_err!(
                "Failed to traverse and update cctx tree relationships: {}",
                e
            )
        })?;

    Ok(inserted)
}

#[instrument(,level="info",skip_all, fields(job_id = %job_id))]
async fn sync_tokens(
    job_id: Uuid,
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    batch_size: u32,
) -> anyhow::Result<()> {
    let mut next_key: Option<String> = None;

    loop {
        let mut response = client.list_tokens(next_key.as_deref(), batch_size).await?;

        // Enrich tokens with icon URLs
        for token in &mut response.foreign_coins {
            if let Some(icon) = client
                .fetch_token_icon(&token.zrc20_contract_address)
                .await
                .unwrap_or(None)
            {
                token.icon_url = Some(icon);
            }
        }

        if response.foreign_coins.is_empty() {
            tracing::debug!("No tokens found");
            break;
        }

        database.sync_tokens(job_id, response.foreign_coins).await?;

        if response.pagination.next_key.is_none() {
            tracing::debug!("Reached end of tokens");
            break;
        }

        next_key = response.pagination.next_key;
    }

    Ok(())
}

fn prio_left(_: &mut ()) -> PollNext {
    PollNext::Left
}
impl Indexer {
    pub fn new(
        settings: IndexerSettings,
        client: Arc<Client>,
        database: Arc<ZetachainCctxDatabase>,
        channel_broadcaster: Arc<ChannelBroadcaster>,
    ) -> Self {
        Self {
            settings,
            client,
            database,
            channel_broadcaster,
        }
    }

    #[instrument(,level="info",skip(self))]
    fn realtime_fetch_handler(&self) -> JoinHandle<()> {
        let realtime_polling_interval = self.settings.realtime_polling_interval;
        let client = self.client.clone();
        let database = self.database.clone();
        let batch_size = self.settings.realtime_fetch_batch_size;
        let broadcaster = self.channel_broadcaster.clone();
        tokio::spawn(async move {
            loop {
                let job_id = Uuid::new_v4();
                match realtime_fetch(job_id, database.clone(), &client, batch_size).await {
                    std::result::Result::Ok(inserted) => {
                        if !inserted.is_empty() {
                            tracing::info!(
                                "realtime_fetch_handler job_id: {} new cctxs found: {:?}",
                                job_id,
                                inserted
                                    .iter()
                                    .map(|c| c.index.clone())
                                    .collect::<Vec<String>>()
                            );
                            broadcaster.broadcast(ChannelEvent::new(
                                NEW_CCTXS_TOPIC,
                                "new_cctxs",
                                &inserted,
                            ));
                        } else {
                            tracing::info!(
                                "realtime_fetch_handler job_id: {} no new cctxs found",
                                job_id
                            );
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, job_id = %job_id, "Failed to fetch realtime data");
                    }
                }
                tokio::time::sleep(Duration::from_millis(realtime_polling_interval)).await;
            }
        })
    }

    #[instrument(,level="debug",skip(self))]
    pub async fn run(&self) -> anyhow::Result<()> {
        tracing::debug!("initializing indexer");
        self.database.setup_db().await?;
        tracing::debug!("setup completed, initializing streams");
        let status_update_batch_size = self.settings.status_update_batch_size;
        let polling_interval = self.settings.polling_interval;
        let status_update_stream = Box::pin(async_stream::stream! {
            loop {
                let batch_id = Uuid::new_v4();
                match self.database.query_cctxs_for_status_update(status_update_batch_size, batch_id, polling_interval).await {
                    std::result::Result::Ok(cctxs) => {
                        tracing::info!("batch_id: {} query_cctxs_for_status_update returned {:?} cctxs for status update", batch_id, cctxs.iter().map(|c| c.index.clone()).collect::<Vec<String>>());
                        for cctx in cctxs {
                            let job_id = Uuid::new_v4();
                            yield IndexerJob::StatusUpdate(cctx, job_id);
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, batch_id = %batch_id, "Failed to query cctxs for status update");
                    }
                }
                tokio::time::sleep(Duration::from_millis(polling_interval)).await;
            }
        });

        // checks whether the realtme fetcher hasn't actually fetched all the data in a single request, so we might be lagging behind
        let level_data_gap_stream = Box::pin(async_stream::stream! {
            loop {
                match self.database.get_unlocked_watermarks(Kind::Realtime).await {
                    std::result::Result::Ok(watermarks) => {
                        for watermark in watermarks.into_iter() {
                            yield IndexerJob::LevelDataGap(watermark.watermark_id,watermark.pointer, watermark.retries_number, Uuid::new_v4());
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to get unlocked watermarks");
                    }
                }
                tokio::time::sleep(Duration::from_millis(self.settings.polling_interval)).await;
            }
        });

        let historical_stream = Box::pin(async_stream::stream! {
            loop {
                let job_id = Uuid::new_v4();
                match self.database.get_unlocked_watermarks(Kind::Historical).await {
                    std::result::Result::Ok(watermarks) => {
                        for watermark in watermarks.into_iter() {
                            yield IndexerJob::HistoricalDataFetch(job_id, HistoricalDataFetchJob {
                                watermark_id: watermark.watermark_id,
                                watermark_pointer: watermark.pointer,
                                watermark_upper_bound_timestamp: watermark.upper_bound_timestamp,
                            });
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, job_id = %job_id, "Failed to get unlocked watermarks");
                    }
                }
                tokio::time::sleep(Duration::from_millis(self.settings.polling_interval)).await;
            }
        });

        let failed_cctxs_stream = Box::pin(async_stream::stream! {
            let batch_size = self.settings.status_update_batch_size;
            let polling_interval = self.settings.failed_cctxs_polling_interval;
            loop {
                let batch_id = Uuid::new_v4();
                match self.database.query_failed_cctxs(batch_id, batch_size, polling_interval).await {
                    std::result::Result::Ok(cctxs) => {
                        for cctx in cctxs {
                            let job_id = Uuid::new_v4();
                            yield IndexerJob::StatusUpdate(cctx, job_id);
                        }
                    }
                    Err(e) => {
                        tracing::error!(error = %e, batch_id = %batch_id, "Failed to query failed cctxs");
                    }
                }
                tokio::time::sleep(Duration::from_millis(self.settings.polling_interval)).await;
            }
        });

        let token_stream = Box::pin(async_stream::stream! {
            loop {
                let job_id = Uuid::new_v4();
                yield IndexerJob::TokenSync(job_id);
                tokio::time::sleep(Duration::from_millis(self.settings.token_polling_interval)).await;
            }
        });

        if self.settings.token_sync_enabled {
            //we need to sync tokens first, because we need to have tokens in the database to process the historical data
            sync_tokens(
                Uuid::new_v4(),
                self.database.clone(),
                &self.client,
                self.settings.token_batch_size,
            )
            .await?;
        }

        //Most of the data is processed in streams, because we don't care about exact timings but rather the eventual consistency and processing order.
        // Priority strategy in descending order:
        // 1. Levelling gaps in the recent data (ensuring data continuity is the highest priority). Failure strategy is unclear for now, we just keep trying. Exhaustion is unlikely.
        // 2. Status updates and related cctxs search (medium priority) in LIFO order. If cctx update fails a lot, we mark it as failed and pass it to the failed cctxs stream.
        // 3. Historical sync. We can't skip the watermark, because it's sequential, and we can't fetch the next pointer without processing the current one, so we have to keep trying.
        // 4. Failed cctx updates (lowest priority)
        // 5. Token synchronization (lowest priority)
        let combined_stream =
            select_with_strategy(historical_stream, failed_cctxs_stream, prio_left);
        let combined_stream =
            select_with_strategy(status_update_stream, combined_stream, prio_left);
        let combined_stream =
            select_with_strategy(level_data_gap_stream, combined_stream, prio_left);
        let combined_stream = select_with_strategy(combined_stream, token_stream, prio_left);
        // Unlike everything else, realtime data fetch must run at configured frequency, so we run it in parallel as a dedicated thread
        // We can't use streams because it would lead to accumulation of jobs
        let dedicated_real_time_fetcher = self.realtime_fetch_handler();

        let streams = combined_stream
            .for_each_concurrent(Some(self.settings.concurrency as usize), |job| {
                let client = self.client.clone();
                let database = self.database.clone();
                let channel_broadcaster = self.channel_broadcaster.clone();
                let historical_batch_size = self.settings.historical_batch_size;
                let level_data_gap_batch_size = self.settings.realtime_fetch_batch_size;
                let retry_threshold = self.settings.retry_threshold;
                let realtime_threshold = self.settings.realtime_threshold;
                let token_batch_size = self.settings.token_batch_size;
                tokio::spawn(async move {
                    match job {
                        IndexerJob::StatusUpdate(cctx, job_id) => {
                            if let Err(e) =  refresh_status_and_link_related(database.clone(), &client, &cctx, job_id, &channel_broadcaster).await {
                                tracing::warn!(error = %e, job_id = %job_id, index = %cctx.index, "Failed to refresh status and link related cctx");
                                if cctx.retries_number == retry_threshold as i32 {
                                    let _ = database.mark_cctx_as_failed(&cctx).await.map_err(|e| anyhow::anyhow!(format!("Failed to mark cctx as failed: {}", e)));
                                } else {
                                    let _ = database.unlock_cctx(cctx.id, job_id).await.map_err(|e| anyhow::anyhow!(format!("Failed to unlock cctx: {}", e)));
                                }
                            }
                        }
                        IndexerJob::LevelDataGap(id,pointer, retries_number, job_id) => {

                            match level_data_gap(job_id, database.clone(), &client, id, pointer, level_data_gap_batch_size, realtime_threshold)
                             .await {
                             std::result::Result::Ok((ProcessingStatus::Done, inserted)) =>  {
                                database.mark_watermark_as_done(id,job_id).await.unwrap();
                                broadcast_new_cctxs_if_any(&channel_broadcaster, &inserted);
                             },
                             std::result::Result::Ok((ProcessingStatus::Unlocked, inserted)) =>  {
                                broadcast_new_cctxs_if_any(&channel_broadcaster, &inserted);
                             },
                             std::result::Result::Err(e) => {
                                tracing::warn!(error = %e, job_id = %job_id, "Failed to level data gap");
                                 if retries_number == retry_threshold as i32 {
                                    let _ = database.mark_watermark_as_failed(id).await.map_err(|e| anyhow::anyhow!(format!("Failed to mark watermark as failed: {}", e)));
                                 } else {
                                    let _ = database.unlock_watermark(id,job_id).await.map_err(|e| anyhow::anyhow!(format!("Failed to unlock watermark: {}", e)));
                                 }
                             }
                             _ => {}
                         }
                        }

                        IndexerJob::HistoricalDataFetch(job_id, job) => {
                            if let Err(e) = historical_sync(database.clone(), &client, job_id, job.clone(), historical_batch_size).await {
                                tracing::warn!(error = %e, job_id = %job_id, watermark_id = %job.watermark_id, watermark_pointer = %job.watermark_pointer, "Failed to sync historical data");
                                database.unlock_watermark(job.watermark_id,job_id).await.unwrap();
                            }
                        }

                        IndexerJob::TokenSync(job_id) => {
                            if let Err(e) = sync_tokens(job_id, database.clone(), &client, token_batch_size).await {
                                tracing::warn!(error = %e, job_id = %job_id, "Failed to sync tokens");
                            }
                        }
                    }
                });
                futures::future::ready(())
            });
        join(dedicated_real_time_fetcher, streams)
            .await
            .0
            .map_err(anyhow::Error::from)
    }
}
