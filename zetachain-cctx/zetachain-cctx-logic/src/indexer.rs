use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Ok;
use futures::future::join;

use tokio::task::JoinHandle;
use uuid::Uuid;
use crate::database::ZetachainCctxDatabase;
use crate::models::{CctxShort, PagedCCTXResponse};
use zetachain_cctx_entity::sea_orm_active_enums::{Kind, ProcessingStatus};
use zetachain_cctx_entity::watermark;

use sea_orm::ColumnTrait;
use std::result::Result::Ok as ResultOk;
use std::result::Result::Err as ResultErr;
use crate::{client::Client, settings::IndexerSettings};
use futures::StreamExt;
use sea_orm::{ActiveValue, DatabaseConnection, EntityTrait, QueryFilter};
use tracing::instrument;

use futures::stream::{select_with_strategy, PollNext};



pub struct Indexer {
    pub settings: IndexerSettings,
    pub db: Arc<DatabaseConnection>,
    pub client: Arc<Client>,
    pub database: Arc<ZetachainCctxDatabase>,
}

enum IndexerJob {
    StatusUpdate(CctxShort, Uuid),             //cctx index and id to be updated
    LevelDataGap(watermark::Model, Uuid), // Watermark (pointer) to the next page of cctxs to be fetched
    HistoricalDataFetch(watermark::Model, Uuid), // Watermark (pointer) to the next page of cctxs to be fetched
}

#[instrument(,level="debug",skip_all)]
async fn refresh_cctx_status(
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    cctx: &CctxShort,
) -> anyhow::Result<()> {
    let fetched_cctx = client.fetch_cctx(&cctx.index).await.map_err(|e| anyhow::anyhow!(format!("Failed to fetch cctx: {}", e)))?;
    database.update_cctx_status(cctx.id, fetched_cctx).await.map_err(|e| anyhow::anyhow!(format!("Failed to update cctx status: {}", e)))?;
    Ok(())
}

#[instrument(,level="info",skip(database, client,watermark), fields(job_id = %job_id, watermark = %watermark.pointer))]
async fn level_data_gap(
    job_id: Uuid,
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    watermark: watermark::Model,
    batch_size: u32,
    realtime_threshold: i64,
) -> anyhow::Result<(ProcessingStatus,String)> {
    let PagedCCTXResponse { cross_chain_tx : cctxs, pagination } = client
    .list_cctxs(Some(&watermark.pointer), false, batch_size)
    .await?;
    
    if cctxs.is_empty() {
        tracing::debug!("No recent cctxs found");
        return Ok((ProcessingStatus::Done,watermark.pointer));
    }
    
    let next_key = pagination.next_key.ok_or(anyhow::anyhow!("next_key is None"))?;
    let res = database.import_missing_cctxs(job_id, cctxs, &next_key, realtime_threshold, watermark).await?;
    tracing::debug!(" process_realtime_cctxs indexer import_missing_cctxs result: {:?}", res);
    Ok(res)
}

#[instrument(,level="info",skip(database, client, watermark), fields(job_id = %job_id, watermark = %watermark.pointer))]
async fn historical_sync(
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    watermark: watermark::Model,
    job_id: Uuid,
    batch_size: u32,
) -> anyhow::Result<()> {
    let response = client
        .list_cctxs(Some(&watermark.pointer), true, batch_size)
        .await?;
    let cross_chain_txs = response.cross_chain_tx;
    if cross_chain_txs.is_empty() {
        return Err(anyhow::anyhow!("Node returned empty response"));
    }
    let next_key = response.pagination.next_key.ok_or(anyhow::anyhow!("next_key is None"))?;
    //atomically insert cctxs and update watermark    
    database.import_cctxs(job_id, cross_chain_txs, &next_key, watermark).await?;
    Ok(())
}

#[instrument(,level="debug",skip(database, client), fields(job_id = %job_id))]
async fn realtime_fetch(job_id: Uuid,database: Arc<ZetachainCctxDatabase>, client: &Client,batch_size: u32) -> anyhow::Result<()> {
    let response = client
        .list_cctxs(None, false, batch_size)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch external data: {}", e))?;
    let txs = response.cross_chain_tx;
    if txs.is_empty() {
        tracing::debug!("No new cctxs found");
        return Ok(());
    }
    let next_key = response.pagination.next_key.ok_or(anyhow::anyhow!("next_key is None"))?;

    database.process_realtime_cctxs(job_id, txs, &next_key).await?;

    Ok(())
}

#[instrument(,level="debug",skip(database, client,cctx), fields(job_id = %job_id, cctx_index = %cctx.index))]
async fn refresh_status_and_link_related(
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    cctx: &CctxShort,
    job_id: Uuid,
) -> anyhow::Result<()> {
    refresh_cctx_status( database.clone(), client, &cctx).await.map_err(|e| anyhow::format_err!("Failed to refresh cctx status: {}", e))?;
    update_cctx_relations( database.clone(), client, &cctx, job_id).await.map_err(|e| anyhow::format_err!("Failed to update cctx relations: {}", e))?;
    Ok(())
}


#[instrument(,level="debug",skip_all)]
async fn update_cctx_relations(
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    cctx: &CctxShort,
    job_id: Uuid,
) -> anyhow::Result<()> {
    // Fetch children using the inbound hash to CCTX data endpoint
    let cross_chain_txs = client
        .get_inbound_hash_to_cctx_data(&cctx.index)
        .await.map_err(|e| anyhow::format_err!("Failed to fetch children: {}", e))?
        .cross_chain_txs;
    
    //turns out that we migh get duplicate cross_chain_txs, so we deduplicate them
    let cctx_map: HashMap<String, _> = cross_chain_txs.into_iter().map(|cctx| (cctx.index.clone(), cctx)).collect();
    let cross_chain_txs = cctx_map.values().cloned().collect::<Vec<_>>();
    
    database
    .traverse_and_update_tree_relationships(cross_chain_txs.clone(), cctx, job_id)
    .await
    .map_err(|e| anyhow::format_err!("Failed to traverse and update cctx tree relationships: {}", e))?;

    Ok(())
}


fn prio_left(_: &mut ()) -> PollNext {
    PollNext::Left
}
impl Indexer {
    pub fn new(
        settings: IndexerSettings,
        db: Arc<DatabaseConnection>,
        client: Arc<Client>,
        database: Arc<ZetachainCctxDatabase>,
    ) -> Self {
        Self {
            settings,
            db,
            client,
            database,
        }
    }

    #[instrument(,level="debug",skip(self))]
    fn realtime_fetch_handler(&self) -> JoinHandle<()> {
        
        let polling_interval = self.settings.polling_interval;
        let client = self.client.clone();
        let database = self.database.clone();
        let batch_size = self.settings.realtime_fetch_batch_size;
        tokio::spawn(async move {
            
            loop {
                let job_id = Uuid::new_v4();
                if let Err(e) = realtime_fetch(job_id, database.clone(), &client, batch_size).await {
                    tracing::error!(error = %e, job_id = %job_id, "Failed to fetch realtime data");
                }
                tokio::time::sleep(Duration::from_millis(polling_interval)).await;
            }
        })
    }

    #[instrument(,level="debug",skip(self,db))]
    async fn acquire_historical_watermark(&self,db: &DatabaseConnection, job_id: Uuid) -> anyhow::Result<watermark::Model> {
        
        let historical_watermark = watermark::Entity::find()
            .filter(watermark::Column::Kind.eq(Kind::Historical))
            .filter(watermark::Column::ProcessingStatus.eq(ProcessingStatus::Unlocked))
            .one(db)
            .await?
            .ok_or_else(|| anyhow::anyhow!("historical watermark is absent or locked"))?;

        self.database.lock_watermark(historical_watermark.clone(),job_id).await?;

        tracing::debug!("acquired historical watermark: {}", historical_watermark.id);
        Ok(historical_watermark)
    }

    #[instrument(,level="debug",skip(self))]
    pub async fn run(&self)-> anyhow::Result<()> {
        if !self.settings.enabled {
            tracing::debug!("indexer is disabled");
            return Ok(());
        }
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
                        tracing::debug!("batch_id: {} query_cctxs_for_status_update returned {:?} cctxs for status update", batch_id, cctxs.iter().map(|c| c.index.clone()).collect::<Vec<String>>());
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
        }
        );

        let db = self.db.clone();
        // checks whether the realtme fetcher hasn't actually fetched all the data in a single request, so we might be lagging behind
        let level_data_gap_stream = Box::pin(async_stream::stream! {
            loop {

                let watermarks = watermark::Entity::find()
                    .filter(watermark::Column::Kind.eq(Kind::Realtime))
                    .filter(watermark::Column::ProcessingStatus.eq(ProcessingStatus::Unlocked))
                    .all(db.as_ref())
                    .await
                    .unwrap();

                for watermark in watermarks {
                    //update watermark lock to true
                    watermark::Entity::update(watermark::ActiveModel {
                        id: ActiveValue::Set(watermark.id),
                        processing_status: ActiveValue::Set(ProcessingStatus::Locked),
                        ..Default::default()
                    })
                    .filter(watermark::Column::Id.eq(watermark.id))
                    .exec(db.as_ref())
                    .await
                    .unwrap();

                    yield IndexerJob::LevelDataGap(watermark, Uuid::new_v4());
                }

                tokio::time::sleep(Duration::from_millis(self.settings.polling_interval)).await;
            }
        });

        let historical_stream = Box::pin(async_stream::stream! {
            loop {
                let job_id = Uuid::new_v4();
                match self.acquire_historical_watermark(&self.db, job_id).await {
                    std::result::Result::Ok(watermark) => {
                        yield IndexerJob::HistoricalDataFetch(watermark, job_id);
                    }
                    Err(e) => {
                        tracing::debug!("job_id: {} failed to acquire historical watermark: {}", job_id, e);
                    }
                }
                tokio::time::sleep(Duration::from_millis(self.settings.polling_interval)).await;
            }
        });

        let failed_cctxs_stream = Box::pin(async_stream::stream! {
            loop {
                let batch_id = Uuid::new_v4();
                match self.database.query_failed_cctxs(batch_id).await {
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


        //Most of the data is processed in streams, because we don't care about exact timings but rather the eventual consistency and processing order.
        // Priority strategy in descending order:
        // 1. Levelling gaps in the recent data (ensuring data continuity is the highest priority). Failure strategy is unclear for now, we just keep trying. Exhaustion is unlikely.
        // 2. Status updates and related cctxs search (medium priority) in LIFO order. If cctx update fails a lot, we mark it as failed and pass it to the failed cctxs stream.
        // 3. Historical sync. We can't skip the watermark, because it's sequential, and we can't fetch the next pointer without processing the current one, so we have to keep trying.
        // 4. Failed cctx updates (lowest priority)
        let combined_stream =
            select_with_strategy(historical_stream, failed_cctxs_stream, prio_left);
        let combined_stream =
            select_with_strategy(status_update_stream, combined_stream, prio_left);
        let combined_stream = select_with_strategy(level_data_gap_stream, combined_stream, prio_left);
        // Unlike everything else, realtime data fetch must run at configured frequency, so we run it in parallel as a dedicated thread 
        // We can't use streams because it would lead to accumulation of jobs
        let dedicated_real_time_fetcher = self.realtime_fetch_handler();
        
        let streams = combined_stream
            .for_each_concurrent(Some(self.settings.concurrency as usize), |job| {
                let client = self.client.clone();
                let database = self.database.clone();
                let historical_batch_size = self.settings.historical_batch_size;
                let level_data_gap_batch_size = self.settings.realtime_fetch_batch_size;
                let retry_threshold = self.settings.retry_threshold;
                let realtime_threshold = self.settings.realtime_threshold;
                tokio::spawn(async move {
                    match job {
                        IndexerJob::StatusUpdate(cctx, job_id) => {
                            if let Err(e) =  refresh_status_and_link_related(database.clone(), &client, &cctx, job_id).await {
                                tracing::info!(error = %e, job_id = %job_id, index = %cctx.index, "Failed to refresh status and link related cctx");
                                if cctx.retries_number == retry_threshold as i32 {
                                    database.mark_cctx_as_failed(&cctx).await.unwrap();
                                } else {
                                database.unlock_cctx(cctx.id, job_id).await.unwrap();
                                }
                            }
                        }
                        IndexerJob::LevelDataGap(watermark, job_id) => {
                           
                            match level_data_gap(job_id, database.clone(), &client, watermark.clone(), level_data_gap_batch_size, realtime_threshold)  
                             .await {
                             ResultOk((ProcessingStatus::Done,_)) =>  database.mark_watermark_as_done(watermark,job_id).await.unwrap(),
                             ResultErr(e) => {
                                 tracing::warn!(error = %e, job_id = %job_id, "Failed to level data gap");
                                 if watermark.retries_number < retry_threshold as i32 {
                                    database.unlock_watermark(watermark,job_id).await.unwrap();
                                 } else {
                                    database.mark_watermark_as_failed(watermark).await.unwrap();
                                 }
                             }
                             _ => {}
                         }
                        }   
                        
                        IndexerJob::HistoricalDataFetch(watermark, job_id) => { 
                            if let Err(e) = historical_sync(database.clone(), &client, watermark.clone(), job_id, historical_batch_size).await {
                                tracing::warn!(error = %e, job_id = %job_id, watermark = %watermark.pointer, "Failed to sync historical data");
                                database.unlock_watermark(watermark,job_id).await.unwrap();
                            }
                        }
                    }
                });
                futures::future::ready(())
            });
        join(dedicated_real_time_fetcher, streams).await.0.map_err(anyhow::Error::from)
        
    
    }
}
