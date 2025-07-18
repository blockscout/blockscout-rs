use std::sync::Arc;

use zetachain_cctx_logic::database::ZetachainCctxDatabase;
use zetachain_cctx_proto::blockscout::zetachain_cctx::v1::{
    token_info_server::TokenInfo, GetTokenInfoRequest, TokenInfoResponse,
};
use tonic::{Request, Response, Status};
use tracing::instrument;

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
    ) -> Result<Response<TokenInfoResponse>, Status> {
        let req = request.into_inner();
        
        if req.asset.is_empty() {
            return Err(Status::invalid_argument("Asset cannot be empty"));
        }

        let token_info = self
            .db
            .get_token_by_asset(&req.asset)
            .await
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                Status::internal("Failed to query token information")
            })?;

        match token_info {
            Some(token) => Ok(Response::new(TokenInfoResponse {
                foreign_chain_id: token.foreign_chain_id,
                decimals: token.decimals,
                name: token.name,
                symbol: token.symbol,
            })),
            None => Err(Status::not_found("Token not found")),
        }
    }
} 