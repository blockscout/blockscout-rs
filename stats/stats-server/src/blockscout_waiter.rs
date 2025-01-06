use std::time::Duration;

use crate::settings::{Settings, StartConditionSettings, ToggleableThreshold};

use anyhow::Context;
use blockscout_service_launcher::launcher::ConfigSettings;
use tokio::time::sleep;
use tracing::{info, warn};

const RETRIES: u64 = 10;

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

pub async fn wait_for_blockscout_indexing(
    api_config: blockscout_client::Configuration,
    wait_config: StartConditionSettings,
) -> Result<(), anyhow::Error> {
    let mut consecutive_errors = 0;
    loop {
        match blockscout_client::apis::main_page_api::get_indexing_status(&api_config).await {
            Ok(result)
                if is_threshold_passed(
                    &wait_config.blocks_ratio,
                    result.indexed_blocks_ratio.clone(),
                    "indexed_blocks_ratio",
                )
                .context("check index block ratio")?
                    && is_threshold_passed(
                        &wait_config.internal_transactions_ratio,
                        result.indexed_internal_transactions_ratio.clone(),
                        "indexed_internal_transactions_ratio",
                    )? =>
            {
                info!("Blockscout indexing threshold passed");
                return Ok(());
            }
            Ok(_) => {
                info!("Blockscout indexing threshold is not passed");
                consecutive_errors = 0;
            }
            Err(e) => {
                if consecutive_errors >= RETRIES {
                    return Err(e).context("Requesting indexing status");
                }
                warn!("Error ({consecutive_errors}/{RETRIES}) requesting indexing status: {e:?}");
                consecutive_errors += 1;
            }
        }
        info!(
            "Rechecking indexing status in {} secs",
            wait_config.check_period_secs
        );
        sleep(Duration::from_secs(wait_config.check_period_secs.into())).await;
    }
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
    use tokio::{task::JoinSet, time::error::Elapsed};
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

    async fn test_wait_indexing(
        wait_config: StartConditionSettings,
        timeout: Option<Duration>,
        response: ResponseTemplate,
    ) -> Result<Result<(), anyhow::Error>, Elapsed> {
        let server = mock_indexing_status(response).await;
        tokio::time::timeout(
            timeout.unwrap_or(Duration::from_millis(500)),
            wait_for_blockscout_indexing(
                blockscout_client::Configuration::new(Url::from_str(&server.uri()).unwrap()),
                wait_config,
            ),
        )
        .await
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
    async fn wait_for_blockscout_indexing_works_with_200_response(
        wait_config: StartConditionSettings,
    ) {
        test_wait_indexing(
            wait_config.clone(),
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

        test_wait_indexing(
            wait_config,
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
    }

    #[rstest]
    #[tokio::test]
    async fn wait_for_blockscout_indexing_works_with_slow_response(
        wait_config: StartConditionSettings,
    ) {
        test_wait_indexing(
            wait_config,
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
    async fn wait_for_blockscout_indexing_works_with_infinite_timeout(
        wait_config: StartConditionSettings,
    ) {
        test_wait_indexing(
            wait_config,
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
    }

    #[rstest]
    #[tokio::test]
    async fn wait_for_blockscout_indexing_works_with_null_ratios(
        wait_config: StartConditionSettings,
    ) {
        test_wait_indexing(
            wait_config,
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
    async fn wait_for_blockscout_indexing_retries_with_error_codes(
        #[with(0.9, 0.9, 1)] wait_config: StartConditionSettings,
    ) {
        let timeout = Some(Duration::from_millis(1500));
        let mut error_servers = JoinSet::from_iter([
            test_wait_indexing(wait_config.clone(), timeout, ResponseTemplate::new(429)),
            test_wait_indexing(wait_config.clone(), timeout, ResponseTemplate::new(500)),
            test_wait_indexing(wait_config.clone(), timeout, ResponseTemplate::new(503)),
            test_wait_indexing(wait_config.clone(), timeout, ResponseTemplate::new(504)),
            test_wait_indexing(wait_config.clone(), timeout, ResponseTemplate::new(400)),
            test_wait_indexing(wait_config.clone(), timeout, ResponseTemplate::new(403)),
            test_wait_indexing(wait_config.clone(), timeout, ResponseTemplate::new(404)),
            test_wait_indexing(wait_config.clone(), timeout, ResponseTemplate::new(405)),
        ]);
        #[allow(for_loops_over_fallibles)]
        for server in error_servers.join_next().await {
            let test_result = server.unwrap();
            test_result.expect_err("must time out");
        }
    }
}
