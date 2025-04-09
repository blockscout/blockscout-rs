use std::time::Duration;

use crate::settings::{Settings, StartConditionSettings, ToggleableThreshold};

use anyhow::Context;
use blockscout_service_launcher::launcher::ConfigSettings;
use stats::indexing_status::{
    BlockscoutIndexingStatus, IndexingStatus, IndexingStatusTrait, UserOpsIndexingStatus,
};
use tokio::{sync::watch, time::sleep};

const RETRIES: u64 = 10;

/// Checks blockscout indexing status and translates it to
/// a `tokio`'s `watch` channel in a convenient form.
///
/// The [`IndexingStatusListener`] contains the other end of
/// the channel. It should be used to actually wait for the
/// status.
///
/// Can be created with [`init`]
pub struct IndexingStatusAggregator {
    api_config: blockscout_client::Configuration,
    wait_config: StartConditionSettings,
    sender: watch::Sender<IndexingStatus>,
}

impl IndexingStatusAggregator {
    fn blockscout_internal_status_from_api_status(
        api_status: blockscout_client::models::IndexingStatus,
        wait_config: &StartConditionSettings,
    ) -> anyhow::Result<BlockscoutIndexingStatus> {
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
                BlockscoutIndexingStatus::InternalTransactionsIndexed
            } else {
                BlockscoutIndexingStatus::BlocksIndexed
            }
        } else {
            BlockscoutIndexingStatus::NoneIndexed
        };
        Ok(status)
    }

    fn user_ops_internal_status_from_api_status(
        api_status: blockscout_client::models::V1IndexerStatus,
        wait_config: &StartConditionSettings,
    ) -> UserOpsIndexingStatus {
        if !wait_config.user_ops_past_indexing_finished.enabled {
            return UserOpsIndexingStatus::PastOperationsIndexed;
        }
        let finished_past_indexing = api_status.finished_past_indexing.unwrap_or_else(|| {
            tracing::info!("Treating `finished_past_indexing=null` as false.",);
            false
        });
        if finished_past_indexing {
            tracing::info!("User ops are fully indexed");
            UserOpsIndexingStatus::PastOperationsIndexed
        } else {
            tracing::info!("User ops are not fully indexed");
            UserOpsIndexingStatus::IndexingPastOperations
        }
    }

    async fn check_blockscout_status(
        &self,
        consecutive_errors: &mut u64,
    ) -> Result<(), anyhow::Error> {
        match blockscout_client::apis::main_page_api::get_indexing_status(&self.api_config).await {
            Ok(result) => {
                *consecutive_errors = 0;
                match Self::blockscout_internal_status_from_api_status(result, &self.wait_config) {
                    Ok(status) => {
                        let modified = self.sender.send_if_modified(|val| {
                            if val.blockscout != status {
                                val.blockscout = status;
                                true
                            } else {
                                false
                            }
                        });
                        if modified {
                            tracing::info!("Observed new indexing status: {:?}", status);
                        } else {
                            tracing::info!("Indexing status is unchanged");
                        }
                    }
                    Err(e) => tracing::error!("{}", e),
                }
            }
            Err(e) => {
                if *consecutive_errors >= RETRIES {
                    return Err(e).context("Requesting blockscout indexing status");
                }
                tracing::warn!(
                    "Error ({consecutive_errors}/{RETRIES}) requesting blockscout indexing status: {e:?}"
                );
                *consecutive_errors += 1;
            }
        }
        Ok(())
    }

    async fn check_user_ops_status(&self) {
        match blockscout_client::apis::proxy_api::get_account_abstraction_status(&self.api_config)
            .await
        {
            Ok(status) => {
                let status =
                    Self::user_ops_internal_status_from_api_status(status, &self.wait_config);
                let modified = self.sender.send_if_modified(|val| {
                    if val.user_ops != status {
                        val.user_ops = status;
                        true
                    } else {
                        false
                    }
                });
                if modified {
                    tracing::info!("Observed new indexing status: {:?}", status);
                } else {
                    tracing::info!("Indexing status is unchanged");
                }
            }
            // Completely normal behaviour
            Err(blockscout_client::Error::ResponseError(response))
                if response.status == reqwest::StatusCode::NOT_IMPLEMENTED =>
            {
                tracing::info!(response_content =? response.content, "User ops are disabled");
            }
            Err(e) => {
                match &e {
                    blockscout_client::Error::ResponseError(bad_request)
                        if bad_request.status == reqwest::StatusCode::BAD_REQUEST =>
                    {
                        tracing::warn!(
                            error =? e,
                            "Got response with HTTP 400. This likely means that blockscout version \
                            is <7.0.0.",
                        );
                    }
                    _ => {
                        tracing::error!(
                            error =? e,
                            "Failed to get user ops indexing status",
                        );
                    }
                }
                // don't need to change if disabled, because it's handled
                // in `init`
                if self.wait_config.user_ops_past_indexing_finished.enabled {
                    tracing::warn!(
                        "User ops related charts are turned off to avoid \
                        incorrect data. Set `STATS__CONDITIONAL_START__USER_OPS_PAST_INDEXING_FINISHED__ENABLED=false` \
                        to ignore this check and update the charts."
                    );
                }
            }
        }
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        if !self.wait_config.blockscout_checks_enabled()
            && !self.wait_config.user_ops_checks_enabled()
        {
            tracing::info!("All indexing status checks are disabled, stopping status checks");
            return Ok(());
        }
        let mut consecutive_errors = 0;
        loop {
            if self.wait_config.blockscout_checks_enabled() {
                self.check_blockscout_status(&mut consecutive_errors)
                    .await?;
            }
            if self.wait_config.user_ops_checks_enabled() {
                self.check_user_ops_status().await;
            }
            let wait_time = if let IndexingStatus::MAX = *self.sender.borrow() {
                self.wait_config.check_period_secs.saturating_mul(10000)
            } else {
                self.wait_config.check_period_secs
            };
            tracing::info!("Rechecking indexing status in {} secs", wait_time);
            sleep(Duration::from_secs(wait_time.into())).await;
        }
    }
}

/// A convenient way to wait for a particular indexing status.
///
/// Requires [`IndexingStatusAggregator`] to run at the same time.
/// Both are created with [`init`].
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
            .wait_for(|value| value.is_requirement_satisfied(&minimal_status))
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
        tracing::info!("Treating `{value_name}=null` as zero.",);
        0.0
    });
    if value < threshold {
        tracing::info!(
            threshold = threshold,
            current_value = value,
            "Threshold for `{value_name}` is not satisfied"
        );
        Ok(false)
    } else {
        tracing::info!(
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
    // enable immediately if the checks are disabled
    let blockscout_init_value = match (
        wait_config.blocks_ratio.enabled,
        wait_config.internal_transactions_ratio.enabled,
    ) {
        (true, _) => BlockscoutIndexingStatus::NoneIndexed,
        (false, true) => BlockscoutIndexingStatus::BlocksIndexed,
        (false, false) => BlockscoutIndexingStatus::InternalTransactionsIndexed,
    };
    let user_ops_init_value = if wait_config.user_ops_past_indexing_finished.enabled {
        UserOpsIndexingStatus::IndexingPastOperations
    } else {
        UserOpsIndexingStatus::PastOperationsIndexed
    };

    let (sender, receiver) = watch::channel(IndexingStatus {
        blockscout: blockscout_init_value,
        user_ops: user_ops_init_value,
    });
    (
        IndexingStatusAggregator {
            api_config,
            wait_config,
            sender,
        },
        IndexingStatusListener { receiver },
    )
}

pub fn init_blockscout_api_client(
    settings: &Settings,
) -> anyhow::Result<Option<blockscout_client::Configuration>> {
    match (settings.ignore_blockscout_api_absence, &settings.blockscout_api_url) {
        (_, Some(blockscout_api_url)) => Ok(Some(blockscout_client::Configuration::new(blockscout_api_url.clone()))),
        (true, None) => {
            tracing::info!(
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
    use stats::tests::mock_blockscout::{mock_blockscout_api, user_ops_status_response_json};
    use std::time::Duration;
    use tokio::{select, task::JoinSet, time::error::Elapsed};
    use url::Url;
    use wiremock::ResponseTemplate;

    use crate::settings::ToggleableCheck;

    use super::*;

    async fn test_aggregator(
        wait_config: StartConditionSettings,
        expected_status: IndexingStatus,
        timeout: Option<Duration>,
        response_blockscout: ResponseTemplate,
        response_user_ops: Option<ResponseTemplate>,
    ) -> Result<Result<(), anyhow::Error>, Elapsed> {
        let timeout = timeout.unwrap_or(Duration::from_millis(2000));
        let server = mock_blockscout_api(response_blockscout, response_user_ops).await;
        let api_config =
            blockscout_client::Configuration::new(Url::from_str(&server.uri()).unwrap());
        let (aggregator, mut listener) = init(api_config, wait_config);
        let aggregator_future = async {
            aggregator.run().await?;
            sleep(timeout).await;
            Ok::<(), anyhow::Error>(())
        };
        let wait_for_listener_timeout = tokio::time::timeout(
            timeout,
            listener.wait_until_status_at_least(expected_status),
        );

        select! {
            res = aggregator_future => {
                panic!("aggregator terminated with error: {res:?}")
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
        #[default(true)] user_ops_check_enabled: bool,
        #[default(0)] check_period_secs: u32,
    ) -> StartConditionSettings {
        StartConditionSettings {
            blocks_ratio: ToggleableThreshold::enabled(blocks),
            internal_transactions_ratio: ToggleableThreshold::enabled(internal_transactions),
            user_ops_past_indexing_finished: ToggleableCheck {
                enabled: user_ops_check_enabled,
            },
            check_period_secs,
        }
    }

    #[rstest]
    #[tokio::test]
    async fn waiter_works_with_200_response(wait_config: StartConditionSettings) {
        test_aggregator(
            wait_config.clone(),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
            },
            None,
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "finished_indexing": true,
                "finished_indexing_blocks": true,
                "indexed_blocks_ratio": "1.00",
                "indexed_internal_transactions_ratio": "1"
            })),
            Some(ResponseTemplate::new(200).set_body_json(user_ops_status_response_json(true))),
        )
        .await
        .expect("must not timeout")
        .expect("must not error");

        test_aggregator(
            wait_config.clone(),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
            },
            None,
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "finished_indexing": false,
                "finished_indexing_blocks": false,
                "indexed_blocks_ratio": "0.80",
                "indexed_internal_transactions_ratio": "0.80"
            })),
            None,
        )
        .await
        .expect_err("must time out");

        test_aggregator(
            wait_config.clone(),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
            },
            None,
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "finished_indexing": false,
                "finished_indexing_blocks": true,
                "indexed_blocks_ratio": "0.80",
                "indexed_internal_transactions_ratio": "1.00"
            })),
            None,
        )
        .await
        .expect_err("must time out");

        test_aggregator(
            wait_config.clone(),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
            },
            None,
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "finished_indexing": true,
                "finished_indexing_blocks": false,
                "indexed_blocks_ratio": "1.00",
                "indexed_internal_transactions_ratio": "0.80"
            })),
            None,
        )
        .await
        .expect_err("must time out");

        test_aggregator(
            wait_config.clone(),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
            },
            None,
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "finished_indexing": true,
                "finished_indexing_blocks": true,
                "indexed_blocks_ratio": "1.00",
                "indexed_internal_transactions_ratio": "1.00"
            })),
            Some(ResponseTemplate::new(200).set_body_json(user_ops_status_response_json(false))),
        )
        .await
        .expect_err("must time out");

        test_aggregator(
            wait_config,
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::BlocksIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
            },
            None,
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "finished_indexing": true,
                "finished_indexing_blocks": false,
                "indexed_blocks_ratio": "1.00",
                "indexed_internal_transactions_ratio": "0.80"
            })),
            Some(ResponseTemplate::new(200).set_body_json(user_ops_status_response_json(true))),
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
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
            },
            Some(Duration::from_millis(500)),
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "finished_indexing": false,
                    "finished_indexing_blocks": false,
                    "indexed_blocks_ratio": "1.0",
                    "indexed_internal_transactions_ratio": "1.0"
                }))
                .set_delay(Duration::from_millis(50)),
            Some(
                ResponseTemplate::new(200)
                    .set_body_json(user_ops_status_response_json(true))
                    .set_delay(Duration::from_millis(50)),
            ),
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
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
            },
            None,
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "finished_indexing": false,
                    "finished_indexing_blocks": false,
                    "indexed_blocks_ratio": "0.80",
                    "indexed_internal_transactions_ratio": "0.80"
                }))
                .set_delay(Duration::MAX),
            Some(ResponseTemplate::new(200).set_body_json(user_ops_status_response_json(true))),
        )
        .await
        .expect_err("must time out");

        test_aggregator(
            wait_config.clone(),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
            },
            None,
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "finished_indexing": false,
                "finished_indexing_blocks": false,
                "indexed_blocks_ratio": "0.80",
                "indexed_internal_transactions_ratio": "0.80"
            })),
            Some(
                ResponseTemplate::new(200)
                    .set_body_json(user_ops_status_response_json(true))
                    .set_delay(Duration::MAX),
            ),
        )
        .await
        .expect_err("must time out");

        test_aggregator(
            wait_config,
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::NoneIndexed,
                user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
            },
            None,
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "finished_indexing": false,
                    "finished_indexing_blocks": false,
                    "indexed_blocks_ratio": "0.80",
                    "indexed_internal_transactions_ratio": "0.80"
                }))
                .set_delay(Duration::MAX),
            Some(
                ResponseTemplate::new(200)
                    .set_body_json(user_ops_status_response_json(true))
                    .set_delay(Duration::MAX),
            ),
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
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::BlocksIndexed,
                user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
            },
            Some(Duration::from_millis(500)),
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "finished_indexing": false,
                "finished_indexing_blocks": false,
                "indexed_blocks_ratio": null,
                "indexed_internal_transactions_ratio": null
            })),
            None,
        )
        .await
        .expect_err("must time out and not fall with error");
    }

    #[rstest]
    #[tokio::test]
    async fn waiter_retries_with_error_codes(
        #[with(0.9, 0.9, true, 1)] wait_config: StartConditionSettings,
    ) {
        let timeout = Some(Duration::from_millis(1500));
        let s = IndexingStatus {
            blockscout: BlockscoutIndexingStatus::BlocksIndexed,
            user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
        };
        let r = |code: u16| ResponseTemplate::new(code);
        let mut error_servers = JoinSet::from_iter([
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(429), None),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(500), None),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(503), None),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(504), None),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(400), None),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(403), None),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(404), None),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(405), None),
        ]);
        while let Some(server) = error_servers.join_next().await {
            let test_result = server.unwrap();
            test_result.expect_err("must time out");
        }
    }

    #[tokio::test]
    async fn waiter_ignores_errors_when_checks_are_disabled() {
        let timeout = Some(Duration::from_millis(2000));
        let s = IndexingStatus::MOST_RESTRICTIVE;
        let r = |code: u16| ResponseTemplate::new(code);
        let ok_b = ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "finished_indexing": true,
            "finished_indexing_blocks": true,
            "indexed_blocks_ratio": "1.00",
            "indexed_internal_transactions_ratio": "1.00"
        }));
        let ok_u = ResponseTemplate::new(200).set_body_json(user_ops_status_response_json(true));
        let config_b_off = StartConditionSettings {
            blocks_ratio: ToggleableThreshold::disabled(),
            internal_transactions_ratio: ToggleableThreshold::disabled(),
            user_ops_past_indexing_finished: ToggleableCheck { enabled: true },
            check_period_secs: 1,
        };
        let config_u_off = StartConditionSettings {
            blocks_ratio: ToggleableThreshold::default(),
            internal_transactions_ratio: ToggleableThreshold::default(),
            user_ops_past_indexing_finished: ToggleableCheck { enabled: false },
            check_period_secs: 1,
        };
        let mut tests = JoinSet::from_iter(
            [
                (&config_b_off, &s, timeout, &r(400), Some(&ok_u)),
                (&config_b_off, &s, timeout, &r(404), Some(&ok_u)),
                (&config_u_off, &s, timeout, &ok_b, Some(&r(400))),
                (&config_u_off, &s, timeout, &ok_b, Some(&r(404))),
            ]
            .map(|(a, b, c, d, e)| test_aggregator(a.clone(), b.clone(), c, d.clone(), e.cloned())),
        );
        while let Some(server) = tests.join_next().await {
            let test_result = server.unwrap();
            test_result
                .expect("must not timeout")
                .expect("must not error");
        }
    }
}
