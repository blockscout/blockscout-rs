use std::sync::Arc;

use crate::proto::{
    stats_server::Stats,
   
};


use tonic::{Request, Response, Status};
use zetachain_cctx_logic::database::ZetachainCctxDatabase;
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::{GetSyncProgressRequest, GetSyncProgressResponse};

pub struct StatsService {
    db: Arc<ZetachainCctxDatabase>,
}

impl StatsService {
    pub fn new(db: Arc<ZetachainCctxDatabase>) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl Stats for StatsService {
    async fn get_sync_progress(&self, _request: Request<GetSyncProgressRequest>) -> Result<Response<GetSyncProgressResponse>, Status> {
        let sync_progress = self.db.get_sync_progress().await.map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(GetSyncProgressResponse {
            historic_watermark_timestamp: sync_progress.historical_watermark_timestamp.and_utc().timestamp(),
            pending_status_updates_count: sync_progress.pending_status_updates_count,
            pending_cctxs_count: sync_progress.pending_cctxs_count,
            realtime_gaps_count: sync_progress.realtime_gaps_count,
        }))
    }
}