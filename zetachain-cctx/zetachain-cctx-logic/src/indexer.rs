use std::sync::Arc;
use std::time::Duration;

use anyhow::Ok;
use futures::future::join;

use tokio::task::JoinHandle;
use uuid::Uuid;
use crate::database::ZetachainCctxDatabase;
use crate::models::PagedCCTXResponse;
use zetachain_cctx_entity::sea_orm_active_enums::{Kind, ProcessingStatus};
use zetachain_cctx_entity::{
    cross_chain_tx, watermark
};

use sea_orm::ColumnTrait;

use crate::{client::Client, settings::IndexerSettings};
use futures::StreamExt;
use sea_orm::{ActiveValue, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use tracing::{instrument, Instrument};

use futures::stream::{select_with_strategy, PollNext};



pub struct Indexer {
    pub settings: IndexerSettings,
    pub db: Arc<DatabaseConnection>,
    pub client: Arc<Client>,
    pub database: Arc<ZetachainCctxDatabase>,
}

enum IndexerJob {
    StatusUpdate(String, i32, Uuid),             //cctx index and id to be updated
    GapFill(watermark::Model, Uuid), // Watermark (pointer) to the next page of cctxs to be fetched
    HistoricalDataFetch(watermark::Model, Uuid), // Watermark (pointer) to the next page of cctxs to be fetched
    FindRelated(cross_chain_tx::Model, Uuid), // Tree traversal job for CCTX parent-child relationships
}

#[instrument(,level="info",skip(database, client), fields(job_id = %job_id, cctx_index = %cctx_index))]
async fn refresh_cctx_status(
    job_id: Uuid,
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    cctx_index: String,
    cctx_id: i32
) -> anyhow::Result<()> {
    let fetched_cctx = client.fetch_cctx(&cctx_index, job_id).await?;
    database.update_cctx_status(job_id, cctx_id, fetched_cctx).await.map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

#[instrument(,level="info",skip(database, client,watermark), fields(job_id = %job_id, watermark = %watermark.pointer))]
async fn gap_fill(
    job_id: Uuid,
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    watermark: watermark::Model,
    batch_size: u32,
) -> anyhow::Result<()> {
    let PagedCCTXResponse { cross_chain_tx : cctxs, pagination } = client.list_cctxs(Some(&watermark.pointer), false, batch_size,job_id).await.unwrap();
    
    let earliest_cctx = cctxs.last().unwrap();
    let last_synced_cctx = database.query_cctx(earliest_cctx.index.clone()).await?;

    if last_synced_cctx.is_some() {
        tracing::debug!("last synced cctx {} is present, skipping", earliest_cctx.index);
        return Ok(());
    }
    let next_key = pagination.next_key.ok_or(anyhow::anyhow!("next_key is None"))?;
    database.gap_fill(job_id, cctxs, &next_key, watermark, batch_size).await?;    
    Ok(())
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
        .list_cctxs(Some(&watermark.pointer), true, batch_size,job_id)
        .await?;
    let cross_chain_txs = response.cross_chain_tx;
    if cross_chain_txs.is_empty() {
        tracing::debug!("No new cctxs found");
        return Ok(());
    }
    let next_key = response.pagination.next_key.ok_or(anyhow::anyhow!("next_key is None"))?;
    //atomically insert cctxs and update watermark
    database.import_cctxs(job_id, cross_chain_txs, &next_key, watermark).await?;
    Ok(())
}



#[instrument(,level="info",skip(database, client), fields(job_id = %job_id))]
async fn realtime_fetch(job_id: Uuid,database: Arc<ZetachainCctxDatabase>, client: &Client,batch_size: u32) -> anyhow::Result<()> {
    let response = client
        .list_cctxs(None, false, batch_size, job_id)
        .await
        .unwrap();
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
async fn update_cctx_relations(
    job_id: Uuid,
    database: Arc<ZetachainCctxDatabase>,
    client: &Client,
    cctx: cross_chain_tx::Model,
) -> anyhow::Result<()> {
    // Fetch children using the inbound hash to CCTX data endpoint
    let children_response = client
        .get_inbound_hash_to_cctx_data(&cctx.index, job_id)
        .await?;
    
    database.traverse_and_update_tree_relationships(children_response.cross_chain_txs, cctx).await?;

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
                realtime_fetch(job_id, database.clone(), &client, batch_size)
                .await
                .unwrap();
                tokio::time::sleep(Duration::from_millis(polling_interval)).await;
            }
        })
    }

    #[instrument(,level="debug",skip(self))]
    pub async fn run(&self)-> anyhow::Result<()> {

        tracing::debug!("initializing indexer");
    
        self.database.setup_db().await?;
    
        tracing::debug!("setup completed, initializing streams");
        let status_update_batch_size = self.settings.status_update_batch_size;
        let status_update_stream = Box::pin(async_stream::stream! {
            loop {

                
                let job_id = Uuid::new_v4();
                let cctxs = self.database.query_cctxs_for_status_update(status_update_batch_size, job_id).await.unwrap();
                if cctxs.is_empty() {
                    tracing::debug!("job_id: {} no cctxs to update", job_id);
                }
                for (cctx_id, cctx_index) in cctxs {
                    yield IndexerJob::StatusUpdate(cctx_index, cctx_id, job_id);
                }
                tokio::time::sleep(Duration::from_millis(self.settings.polling_interval)).await;
                
            }
        });

        let db = self.db.clone();
        // checks whether the realtme fetcher hasn't actually fetched all the data in a single request, so we might be lagging behind
        let gap_fill_stream = Box::pin(async_stream::stream! {
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

                    yield IndexerJob::GapFill(watermark, Uuid::new_v4());
                }

                tokio::time::sleep(Duration::from_millis(self.settings.polling_interval)).await;
            }
        });

        let db = self.db.clone();
        let historical_stream = Box::pin(async_stream::stream! {
            loop {
                let job_id = Uuid::new_v4();
                let watermarks = watermark::Entity::find()
                    .filter(watermark::Column::Kind.eq(Kind::Historical))
                    .filter(watermark::Column::ProcessingStatus.eq(ProcessingStatus::Unlocked))
                    .one(db.as_ref())
                    .instrument(tracing::debug_span!("looking for historical watermark",job_id = %job_id))
                    .await
                    .unwrap();

                    //TODO: add updated_by field to watermark model
                if let Some(watermark) = watermarks {
                    self.database.lock_watermark(watermark.clone()).await.unwrap();
                    yield IndexerJob::HistoricalDataFetch(watermark, job_id);
                } else {
                    tracing::debug!("job_id: {} historical watermark is absent or locked", job_id);
                }
                

                tokio::time::sleep(Duration::from_millis(self.settings.polling_interval)).await;
            }
        });

        let db = self.db.clone();
        let tree_batch_size = self.settings.status_update_batch_size; // Reuse batch size setting
        let update_cctx_relations_stream = Box::pin(async_stream::stream! {
            loop {
                let job_id = Uuid::new_v4();
                
                // Find CCTXs that haven't been processed for tree traversal
                let unprocessed_cctxs = cross_chain_tx::Entity::find()
                    .filter(cross_chain_tx::Column::TreeQueryFlag.eq(false))
                    .filter(cross_chain_tx::Column::ProcessingStatus.eq(ProcessingStatus::Unlocked))
                    .limit(tree_batch_size as u64)
                    .all(db.as_ref())
                    .await
                    .unwrap();
  
                if unprocessed_cctxs.is_empty() {
                    tracing::debug!("job_id: {} no cctxs to process for tree traversal", job_id);
                } else {
                    for cctx in unprocessed_cctxs {
                        // Lock the CCTX to prevent concurrent processing
                        cross_chain_tx::Entity::update(cross_chain_tx::ActiveModel {
                            id: ActiveValue::Set(cctx.id),
                            processing_status: ActiveValue::Set(ProcessingStatus::Locked),
                            ..Default::default()
                        })
                        .filter(cross_chain_tx::Column::Id.eq(cctx.id))
                        .exec(db.as_ref())
                        .await
                        .unwrap();
    
                        yield IndexerJob::FindRelated(cctx, job_id);
                    }
                }

                tokio::time::sleep(Duration::from_millis(self.settings.polling_interval)).await;
            }
        });

        // Priority strategy:
        // 1. Gap fill (highest priority - if there's a gap, we're lagging behind)
        // 2. Status update for new cctxs (medium priority)
        // 3. Tree traversal (medium priority)
        // 4. Historical sync (lowest priority - can lag without affecting realtime)
        let combined_stream =
            select_with_strategy(status_update_stream, update_cctx_relations_stream, prio_left);
        let combined_stream = select_with_strategy(combined_stream, historical_stream, prio_left);
        let combined_stream = select_with_strategy(gap_fill_stream, combined_stream, prio_left);
        

        // Realtime data fetch must run at configured frequency, so we run it in parallel as a dedicated thread 
        let realtime_handler = self.realtime_fetch_handler();
        
        let streaming = combined_stream
            .for_each_concurrent(Some(self.settings.concurrency as usize), |job| {
                let client = self.client.clone();
                let database = self.database.clone();
                let historical_batch_size = self.settings.historical_batch_size;
                let gap_fill_batch_size = self.settings.realtime_fetch_batch_size;
                tokio::spawn(async move {
                    match job {
                        IndexerJob::StatusUpdate(cctx_index, cctx_id, job_id) => {
                            if let Err(e) =  refresh_cctx_status(job_id, database.clone(), &client, cctx_index, cctx_id)
                            .await {
                                tracing::error!(error = %e, job_id = %job_id, "Failed to update cctx status");
                                database.unlock_cctx(cctx_id).await.unwrap();
                            }
                        }
                        IndexerJob::GapFill(watermark, job_id) => {
                           
                           gap_fill(job_id, database.clone(), &client, watermark.clone(), gap_fill_batch_size).await.unwrap();
                           
                        }
                        IndexerJob::HistoricalDataFetch(watermark, job_id) => { 
                            historical_sync(database.clone(), &client, watermark.clone(), job_id, historical_batch_size).await.unwrap();
                        }
                        IndexerJob::FindRelated(cctx, job_id) => {
                            let cctx_id = cctx.id;
                            if let Err(e) = update_cctx_relations(job_id, database.clone(), &client, cctx).await {
                                tracing::error!(error = %e, job_id = %job_id, "Failed to perform tree traversal");
                                database.unlock_cctx(cctx_id).await.unwrap();
                            }
                        }
                    }
                });
                futures::future::ready(())
            });
        join(realtime_handler, streaming).await.0.map_err(anyhow::Error::from)
        
    
    }
}
