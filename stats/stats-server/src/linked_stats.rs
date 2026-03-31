use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use stats_proto::blockscout::stats::v1 as proto_v1;
use thiserror::Error;
use url::Url;

use crate::settings::LinkedStatsSettings;

pub const LINK_HOP_HEADER: &str = "x-stats-link-hop";

#[derive(Debug, Clone)]
pub struct LinkedStatsClient {
    client: reqwest::Client,
    base_url: Url,
}

#[derive(Debug, Error)]
pub enum LinkedStatsError {
    #[error("linked stats service returned not found")]
    NotFound,
    #[error("linked stats request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("linked stats returned unexpected status {0}")]
    UnexpectedStatus(StatusCode),
}

impl LinkedStatsClient {
    pub fn new(settings: LinkedStatsSettings) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder()
            .timeout(settings.timeout())
            .build()?;
        Ok(Self {
            client,
            base_url: settings.base_url,
        })
    }

    pub async fn get_counters(&self, hop: u32) -> Result<proto_v1::Counters, LinkedStatsError> {
        self.get_json("api/v1/counters", hop).await
    }

    pub async fn get_line_charts(
        &self,
        hop: u32,
    ) -> Result<proto_v1::LineCharts, LinkedStatsError> {
        self.get_json("api/v1/lines", hop).await
    }

    pub async fn get_line_chart(
        &self,
        request: &proto_v1::GetLineChartRequest,
        hop: u32,
    ) -> Result<proto_v1::LineChart, LinkedStatsError> {
        let mut url = self.endpoint(&format!("api/v1/lines/{}", request.name))?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(from) = request.from.as_deref() {
                query.append_pair("from", from);
            }
            if let Some(to) = request.to.as_deref() {
                query.append_pair("to", to);
            }
            let resolution = request.resolution();
            if resolution != proto_v1::Resolution::Unspecified {
                query.append_pair("resolution", resolution.as_str_name());
            }
        }
        self.get_json_by_url(url, hop, true).await
    }

    pub async fn get_main_page_stats(
        &self,
        hop: u32,
    ) -> Result<proto_v1::MainPageStats, LinkedStatsError> {
        self.get_json("api/v1/pages/main", hop).await
    }

    pub async fn get_transactions_page_stats(
        &self,
        hop: u32,
    ) -> Result<proto_v1::TransactionsPageStats, LinkedStatsError> {
        self.get_json("api/v1/pages/transactions", hop).await
    }

    pub async fn get_contracts_page_stats(
        &self,
        hop: u32,
    ) -> Result<proto_v1::ContractsPageStats, LinkedStatsError> {
        self.get_json("api/v1/pages/contracts", hop).await
    }

    pub async fn get_main_page_multichain_stats(
        &self,
        hop: u32,
    ) -> Result<proto_v1::MainPageMultichainStats, LinkedStatsError> {
        self.get_json("api/v1/pages/multichain/main", hop).await
    }

    pub async fn get_main_page_interchain_stats(
        &self,
        hop: u32,
    ) -> Result<proto_v1::MainPageInterchainStats, LinkedStatsError> {
        self.get_json("api/v1/pages/interchain/main", hop).await
    }

    pub async fn get_update_status(
        &self,
        hop: u32,
    ) -> Result<proto_v1::UpdateStatus, LinkedStatsError> {
        self.get_json("api/v1/update-status", hop).await
    }

    fn endpoint(&self, path: &str) -> Result<Url, LinkedStatsError> {
        self.base_url
            .join(path)
            .map_err(|_| LinkedStatsError::UnexpectedStatus(StatusCode::BAD_REQUEST))
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        hop: u32,
    ) -> Result<T, LinkedStatsError> {
        let url = self.endpoint(path)?;
        self.get_json_by_url(url, hop, false).await
    }

    async fn get_json_by_url<T: DeserializeOwned>(
        &self,
        url: Url,
        hop: u32,
        allow_not_found: bool,
    ) -> Result<T, LinkedStatsError> {
        let response = self
            .client
            .get(url)
            .header(LINK_HOP_HEADER, hop.to_string())
            .send()
            .await?;
        let status = response.status();
        if status.is_success() {
            return Ok(response.json().await?);
        }
        if allow_not_found && status == StatusCode::NOT_FOUND {
            return Err(LinkedStatsError::NotFound);
        }
        Err(LinkedStatsError::UnexpectedStatus(status))
    }
}
