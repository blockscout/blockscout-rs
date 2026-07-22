use std::sync::Arc;

use tonic::{Request, Response, Status};
use tracing::instrument;
use zetachain_cctx_logic::database::ZetachainCctxDatabase;
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::{
    token_info_server::TokenInfo, GetTokenInfoRequest, ListTokensRequest, Token as TokenProto,
    Tokens,
};

pub struct TokenInfoService {
    db: Arc<ZetachainCctxDatabase>,
}

impl TokenInfoService {
    pub fn new(db: Arc<ZetachainCctxDatabase>) -> Self {
        Self { db }
    }
}

#[tonic::async_trait]
impl TokenInfo for TokenInfoService {
    #[instrument(level = "debug", skip_all, fields(asset = %request.get_ref().asset))]
    async fn get_token_info(
        &self,
        request: Request<GetTokenInfoRequest>,
    ) -> Result<Response<TokenProto>, Status> {
        let req = request.into_inner();

        if req.asset.is_empty() {
            return Err(Status::invalid_argument("Asset cannot be empty"));
        }

        let token = self
            .db
            .get_token_by_asset(&req.asset)
            .await
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                Status::internal("Failed to query token information")
            })?
            .ok_or(Status::not_found("Token not found"))?;

        Ok(Response::new(token))
    }

    #[instrument(level = "debug", skip_all)]
    async fn list_tokens(
        &self,
        _request: Request<ListTokensRequest>,
    ) -> Result<Response<Tokens>, Status> {
        let tokens = self.db.list_tokens().await.map_err(|e| {
            tracing::error!("Database error: {}", e);
            Status::internal("Failed to query token information")
        })?;

        Ok(Response::new(Tokens { tokens }))
    }
}
