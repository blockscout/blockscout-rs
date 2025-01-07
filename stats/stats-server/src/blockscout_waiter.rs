use std::time::Duration;

use crate::settings::{Settings, StartConditionSettings, ToggleableThreshold};

use anyhow::Context;
use blockscout_service_launcher::launcher::ConfigSettings;
use stats::IndexingStatus;
use tokio::{sync::watch, time::sleep};
use tracing::{info, warn};

const RETRIES: u64 = 10;

pub struct IndexingStatusAggregator {
    api_config: blockscout_client::Configuration,
    wait_config: StartConditionSettings,
    sender: watch::Sender<IndexingStatus>,
}

impl IndexingStatusAggregator {
    fn internal_status_from_api_status(
        api_status: blockscout_client::models::IndexingStatus,
        wait_config: &StartConditionSettings,
    ) -> anyhow::Result<IndexingStatus> {
        let blocks_passed = is_threshold_passed(
            &wait_config.blocks_ratio,
            api_status.indexed_blocks_ratio.clone(),
            "indexed_blocks_ratio",
        )
        .context("checking indexed block ratio")?;
        let status = if blocks_passed {
            let internal_transactions_passed = is_threshold_passed(
                &wait_config.internal_transactions_ratio,
                api_status.indexed_internal_transactions_ratio.clone(),
                "indexed_internal_transactions_ratio",
            )
            .context("checking indexed internal transactions ratio")?;
            if internal_transactions_passed {
                IndexingStatus::InternalTransactionsIndexed
            } else {
                IndexingStatus::BlocksIndexed
            }
        } else {
            IndexingStatus::NoneIndexed
        };
        Ok(status)
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let mut consecutive_errors = 0;
        loop {
            match blockscout_client::apis::main_page_api::get_indexing_status(&self.api_config)
                .await
            {
                Ok(result) => {
                    consecutive_errors = 0;
                    match Self::internal_status_from_api_status(result, &self.wait_config) {
                        Ok(status) => {
                            let modified = self.sender.send_if_modified(|val| {
                                if val != &status {
                                    *val = status.clone();
                                    true
                                } else {
                                    false
                                }
                            });
                            if modified {
                                info!("Observed new indexing status: {:?}", status);
                            } else {
                                info!("Indexing status is unchanged");
                            }
                        }
                        Err(e) => tracing::error!("{}", e),
                    }
                }
                Err(e) => {
                    if consecutive_errors >= RETRIES {
                        return Err(e).context("Requesting indexing status");
                    }
                    warn!(
                        "Error ({consecutive_errors}/{RETRIES}) requesting indexing status: {e:?}"
                    );
                    consecutive_errors += 1;
                }
            }
            info!(
                "Rechecking indexing status in {} secs",
                self.wait_config.check_period_secs
            );
            sleep(Duration::from_secs(
                self.wait_config.check_period_secs.into(),
            ))
            .await;
        }
    }
}

#[derive(Clone)]
pub struct IndexingStatusListener {
    receiver: watch::Receiver<IndexingStatus>,
}

impl IndexingStatusListener {
    pub async fn wait_until_status_at_least(
        &mut self,
        minimal_status: IndexingStatus,
    ) -> Result<(), watch::error::RecvError> {
        self.receiver
            .wait_for(|value| match minimal_status {
                IndexingStatus::NoneIndexed => true,
                IndexingStatus::BlocksIndexed => matches!(
                    value,
                    IndexingStatus::BlocksIndexed | IndexingStatus::InternalTransactionsIndexed
                ),
                IndexingStatus::InternalTransactionsIndexed => {
                    matches!(value, IndexingStatus::InternalTransactionsIndexed)
                }
            })
            .await?;
        Ok(())
    }
}

fn is_threshold_passed(
    threshold: &ToggleableThreshold,
    float_value: Option<String>,
    value_name: &str,
) -> Result<bool, anyhow::Error> {
    let threshold = if threshold.enabled {
        threshold.threshold
    } else {
        return Ok(true);
    };
    let value = float_value
        .map(|s| s.parse::<f64>())
        .transpose()
        .context(format!("Parsing `{value_name}`"))?;
    let value = value.unwrap_or_else(|| {
        info!("Treating `{value_name}=null` as zero.",);
        0.0
    });
    if value < threshold {
        info!(
            threshold = threshold,
            current_value = value,
            "Threshold for `{value_name}` is not satisfied"
        );
        Ok(false)
    } else {
        info!(
            threshold = threshold,
            current_value = value,
            "Threshold for `{value_name}` is satisfied"
        );
        Ok(true)
    }
}

pub fn init(
    api_config: blockscout_client::Configuration,
    wait_config: StartConditionSettings,
) -> (IndexingStatusAggregator, IndexingStatusListener) {
    let (sender, receiver) = watch::channel(IndexingStatus::LEAST_RESTRICTIVE);
    (
        IndexingStatusAggregator {
            api_config,
            wait_config,
            sender,
        },
        IndexingStatusListener { receiver },
    )
}

pub async fn init_blockscout_api_client(
    settings: &Settings,
) -> anyhow::Result<Option<blockscout_client::Configuration>> {
    match (settings.ignore_blockscout_api_absence, &settings.blockscout_api_url) {
        (_, Some(blockscout_api_url)) => Ok(Some(blockscout_client::Configuration::new(blockscout_api_url.clone()))),
        (true, None) => {
            info!(
                "Blockscout API URL has not been provided and `IGNORE_BLOCKSCOUT_API_ABSENCE` setting is \
                set to `true`. Disabling API-related functionality."
            );
            Ok(None)
        }
        (false, None) => anyhow::bail!(
            "Blockscout API URL has not been provided. Please specify it with corresponding \
            env variable (`{0}__BLOCKSCOUT_API_URL`) or set `{0}__IGNORE_BLOCKSCOUT_API_ABSENCE=true` to disable \
            functionality depending on the API.",
            Settings::SERVICE_NAME
        ),
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use rstest::*;
    use std::time::Duration;
    use tokio::{select, task::JoinSet, time::error::Elapsed};
    use url::Url;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    use super::*;

    async fn mock_indexing_status(response: ResponseTemplate) -> MockServer {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/main-page/indexing-status"))
            .respond_with(response)
            .mount(&mock_server)
            .await;
        mock_server
    }

    async fn test_aggregator(
        wait_config: StartConditionSettings,
        expected_status: IndexingStatus,
        timeout: Option<Duration>,
        response: ResponseTemplate,
    ) -> Result<Result<(), anyhow::Error>, Elapsed> {
        let server = mock_indexing_status(response).await;
        let api_config =
            blockscout_client::Configuration::new(Url::from_str(&server.uri()).unwrap());
        let (aggregator, mut listener) = init(api_config, wait_config);
        let wait_for_listener_timeout = tokio::time::timeout(
            timeout.unwrap_or(Duration::from_millis(200)),
            listener.wait_until_status_at_least(expected_status),
        );
        select! {
            res = aggregator.run() => {
                panic!("aggregator terminated: {:?}", res)
            }
            listener = wait_for_listener_timeout => {
                listener.map(|a| a.map_err(|e| e.into()))
            }
        }
    }

    #[fixture]
    fn wait_config(
        #[default(0.9)] blocks: f64,
        #[default(0.9)] internal_transactions: f64,
        #[default(0)] check_period_secs: u32,
    ) -> StartConditionSettings {
        StartConditionSettings {
            blocks_ratio: ToggleableThreshold::enabled(blocks),
            internal_transactions_ratio: ToggleableThreshold::enabled(internal_transactions),
            check_period_secs,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn waiter_works_with_200_response(wait_config: StartConditionSettings) {
        test_aggregator(
            wait_config.clone(),
            IndexingStatus::InternalTransactionsIndexed,
            None,
            ResponseTemplate::new(200).set_body_string(
                r#"{
                    "finished_indexing": true,
                    "finished_indexing_blocks": true,
                    "indexed_blocks_ratio": "1.00",
                    "indexed_internal_transactions_ratio": "1"
                }"#,
            ),
        )
        .await
        .expect("must not timeout")
        .expect("must not error");

        test_aggregator(
            wait_config.clone(),
            IndexingStatus::InternalTransactionsIndexed,
            None,
            ResponseTemplate::new(200).set_body_string(
                r#"{
                    "finished_indexing": false,
                    "finished_indexing_blocks": false,
                    "indexed_blocks_ratio": "0.80",
                    "indexed_internal_transactions_ratio": "0.80"
                }"#,
            ),
        )
        .await
        .expect_err("must time out");

        test_aggregator(
            wait_config.clone(),
            IndexingStatus::InternalTransactionsIndexed,
            None,
            ResponseTemplate::new(200).set_body_string(
                r#"{
                    "finished_indexing": false,
                    "finished_indexing_blocks": true,
                    "indexed_blocks_ratio": "0.80",
                    "indexed_internal_transactions_ratio": "1.00"
                }"#,
            ),
        )
        .await
        .expect_err("must time out");

        test_aggregator(
            wait_config.clone(),
            IndexingStatus::InternalTransactionsIndexed,
            None,
            ResponseTemplate::new(200).set_body_string(
                r#"{
                    "finished_indexing": true,
                    "finished_indexing_blocks": false,
                    "indexed_blocks_ratio": "1.00",
                    "indexed_internal_transactions_ratio": "0.80"
                }"#,
            ),
        )
        .await
        .expect_err("must time out");

        test_aggregator(
            wait_config,
            IndexingStatus::BlocksIndexed,
            None,
            ResponseTemplate::new(200).set_body_string(
                r#"{
                    "finished_indexing": true,
                    "finished_indexing_blocks": false,
                    "indexed_blocks_ratio": "1.00",
                    "indexed_internal_transactions_ratio": "0.80"
                }"#,
            ),
        )
        .await
        .expect("must not timeout")
        .expect("must not error");
    }

    #[rstest]
    #[tokio::test]
    async fn waiter_works_with_slow_response(wait_config: StartConditionSettings) {
        test_aggregator(
            wait_config,
            IndexingStatus::InternalTransactionsIndexed,
            None,
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{
                        "finished_indexing": false,
                        "finished_indexing_blocks": false,
                        "indexed_blocks_ratio": "1.0",
                        "indexed_internal_transactions_ratio": "1.0"
                    }"#,
                )
                .set_delay(Duration::from_millis(100)),
        )
        .await
        .expect("must not timeout")
        .expect("must not error")
    }

    #[rstest]
    #[tokio::test]
    async fn waiter_works_with_infinite_timeout(wait_config: StartConditionSettings) {
        test_aggregator(
            wait_config.clone(),
            IndexingStatus::InternalTransactionsIndexed,
            None,
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{
                        "finished_indexing": false,
                        "finished_indexing_blocks": false,
                        "indexed_blocks_ratio": "0.80",
                        "indexed_internal_transactions_ratio": "0.80"
                    }"#,
                )
                .set_delay(Duration::MAX),
        )
        .await
        .expect_err("must time out");

        test_aggregator(
            wait_config,
            IndexingStatus::NoneIndexed,
            None,
            ResponseTemplate::new(200)
                .set_body_string(
                    r#"{
                        "finished_indexing": false,
                        "finished_indexing_blocks": false,
                        "indexed_blocks_ratio": "0.80",
                        "indexed_internal_transactions_ratio": "0.80"
                    }"#,
                )
                .set_delay(Duration::MAX),
        )
        .await
        .expect("must not timeout")
        .expect("must not error");
    }

    #[rstest]
    #[tokio::test]
    async fn waiter_works_with_null_ratios(wait_config: StartConditionSettings) {
        test_aggregator(
            wait_config,
            IndexingStatus::BlocksIndexed,
            Some(Duration::from_millis(300)),
            ResponseTemplate::new(200).set_body_string(
                r#"{
                    "finished_indexing": false,
                    "finished_indexing_blocks": false,
                    "indexed_blocks_ratio": null,
                    "indexed_internal_transactions_ratio": null
                }"#,
            ),
        )
        .await
        .expect_err("must time out and not fall with error");
    }

    #[rstest]
    #[tokio::test]
    async fn waiter_retries_with_error_codes(
        #[with(0.9, 0.9, 1)] wait_config: StartConditionSettings,
    ) {
        let timeout = Some(Duration::from_millis(1500));
        let s = IndexingStatus::BlocksIndexed;
        let r = |code: u16| ResponseTemplate::new(code);
        let mut error_servers = JoinSet::from_iter([
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(429)),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(500)),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(503)),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(504)),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(400)),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(403)),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(404)),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(405)),
        ]);
        #[allow(for_loops_over_fallibles)]
        for server in error_servers.join_next().await {
            let test_result = server.unwrap();
            test_result.expect_err("must time out");
        }
    }
}
