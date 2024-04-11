use crate::{
    proto::{
        verifier_alliance_server, VerifierAllianceBatchImportResponse,
        VerifierAllianceBatchImportSolidityStandardJsonRequest,
    },
    services::verifier_base,
};
use async_trait::async_trait;
use eth_bytecode_db::verification::{verifier_alliance_handler, Client};
use std::collections::HashSet;
use tonic::{Request, Response, Status};

pub struct VerifierAllianceService {
    client: Client,
    authorized_keys: HashSet<String>,
}

impl VerifierAllianceService {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            authorized_keys: Default::default(),
        }
    }

    pub fn with_authorized_keys(mut self, authorized_keys: HashSet<String>) -> Self {
        self.authorized_keys = authorized_keys;
        self
    }
}

#[async_trait]
impl verifier_alliance_server::VerifierAlliance for VerifierAllianceService {
    async fn batch_import_solidity_standard_json(
        &self,
        request: Request<VerifierAllianceBatchImportSolidityStandardJsonRequest>,
    ) -> Result<Response<VerifierAllianceBatchImportResponse>, Status> {
        if self.client.alliance_db_client.is_none() {
            return Err(Status::unavailable("Verifier alliance is disabled"));
        }

        let (metadata, _, request) = request.into_parts();

        let is_authorized = super::is_key_authorized(&self.authorized_keys, metadata)?;
        if !is_authorized {
            return Err(Status::unauthenticated("api-key is required"));
        }

        let result = verifier_alliance_handler::import_solidity_standard_json(
            self.client.clone(),
            request.try_into()?,
        )
        .await
        .map_err(verifier_base::process_batch_import_error)?;

        Ok(Response::new(result.try_into()?))
    }
}
