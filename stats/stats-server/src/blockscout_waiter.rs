// SPDX-License-Identifier: LicenseRef-Blockscout

use std::{collections::BTreeSet, sync::Arc, time::Duration};

use crate::{
    interchain_indexer_api::{
        InterchainIndexerApiClient, is_catchup_progress_sufficient, slice_catchup_progress,
    },
    settings::{Settings, StartConditionSettings, ToggleableThreshold},
};

use anyhow::Context;
use blockscout_service_launcher::launcher::ConfigSettings;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use stats::{
    indexing_status::{
        BlockscoutIndexingStatus, IndexingStatus, IndexingStatusTrait, InterchainIndexingStatus,
        UserOpsIndexingStatus, ZetachainCctxIndexingStatus,
    },
    query_zetachain_cctx_indexed_until,
    utils::day_start,
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
    api_config: Option<blockscout_client::Configuration>,
    zetachain_cctx_db: Option<Arc<DatabaseConnection>>,
    /// `Some` iff `STATS__INTERCHAIN_INDEXER_API_URL` is configured.
    interchain_api: Option<InterchainCatchupSource>,
    wait_config: StartConditionSettings,
    sender: watch::Sender<IndexingStatus>,
}

/// Everything [`IndexingStatusAggregator::check_interchain_status`] needs,
/// bundled so [`init`]'s parameter list stays readable and so this scope
/// cannot silently drift from the per-cycle verdict's.
pub struct InterchainCatchupSource {
    pub client: Arc<InterchainIndexerApiClient>,
    /// `None` ⇒ unbounded, exactly as in the verdict.
    pub relevant_bridges: Option<Vec<i32>>,
    /// `None` ⇒ unbounded, exactly as in the verdict.
    pub relevant_chains: Option<BTreeSet<i64>>,
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

    fn zetachain_cctx_internal_status_from_db_timestamp(
        indexed_until: Option<DateTime<Utc>>,
        wait_config: &StartConditionSettings,
    ) -> ZetachainCctxIndexingStatus {
        if !wait_config.zetachain_checks_enabled() {
            return ZetachainCctxIndexingStatus::IndexedHistoricalData;
        }
        let today = day_start(&chrono::Utc::now().date_naive());
        match (indexed_until, today) {
            (None, _) => {
                tracing::info!(
                    "No historical watermark timestamp was found in Zetachain CCTX DB, \
                    the zetachain charts are disabled"
                );
                ZetachainCctxIndexingStatus::CatchingUp
            }
            (Some(indexed_until), today) if indexed_until < today => {
                tracing::info!(
                    "Zetachain CCTX is not indexed until today (indexed_until < today_start)=({indexed_until} < {today})"
                );
                ZetachainCctxIndexingStatus::CatchingUp
            }
            (_, _) => ZetachainCctxIndexingStatus::IndexedHistoricalData,
        }
    }

    async fn check_blockscout_status(
        &self,
        consecutive_errors: &mut u64,
    ) -> Result<(), anyhow::Error> {
        let api_config = self.api_config.as_ref().expect(
            "check_blockscout_status is only called while `blockscout_live`, i.e. `api_config.is_some()`",
        );
        match blockscout_client::apis::main_page_api::get_indexing_status(api_config).await {
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
        let api_config = self.api_config.as_ref().expect(
            "check_user_ops_status is only called while `blockscout_live`, i.e. `api_config.is_some()`",
        );
        match blockscout_client::apis::proxy_api::get_account_abstraction_status(api_config).await {
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

    /// Checks if the Zetachain CCTX is indexed until today.
    async fn check_zetachain_status(&self) {
        let Some(db) = self.zetachain_cctx_db.as_deref() else {
            tracing::error!(
                "Zetachain CCTX DB is not connected, cannot check Zetachain CCTX index status.\
                Either connect the db or disable the zetachain indexing status check."
            );
            return;
        };
        let db_watermark_timestamp = match query_zetachain_cctx_indexed_until(db).await {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(error =? e, "Failed to get Zetachain CCTX indexing status");
                return;
            }
        };
        let status = Self::zetachain_cctx_internal_status_from_db_timestamp(
            db_watermark_timestamp,
            &self.wait_config,
        );
        let modified = self.sender.send_if_modified(|val| {
            if val.zetachain_cctx != status {
                val.zetachain_cctx = status;
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

    /// Checks whether the interchain indexer has scanned enough of the configured
    /// slice, per `STATS__CONDITIONAL_START__INTERCHAIN_CATCHUP_MIN_PROGRESS`.
    ///
    /// Failure policy mirrors [`Self::check_blockscout_status`] and deliberately
    /// **not** [`Self::check_zetachain_status`]: a transient error leaves the
    /// `watch` value alone (so the axis keeps its last known state and groups keep
    /// waiting), logs, and retries; `RETRIES` consecutive errors propagate and
    /// stop the aggregator. Warning-and-returning forever is what leaves the
    /// zetachain axis stuck at `CatchingUp` (.memory-bank/gotchas.md:546-555).
    ///
    /// Note this resolves an unreachable API in the **opposite** direction to the
    /// per-cycle catch-up verdict, which treats unknown as "do not force". That
    /// is intentional: this is a start condition, so "pass on error" would make an
    /// enabled check meaningless the moment the indexer blipped, while the
    /// verdict's "block on error" would freeze chart updates in a running
    /// service. Do not unify the two.
    async fn check_interchain_status(
        &self,
        consecutive_errors: &mut u64,
    ) -> Result<(), anyhow::Error> {
        let Some(source) = self.interchain_api.as_ref() else {
            // defensive only: work item 14's startup `bail!` makes "check
            // enabled with no configured source" unreachable.
            tracing::error!(
                "interchain catch-up check is enabled but no InterchainCatchupSource was \
                 configured; this is a bug, the startup validation should have refused to boot"
            );
            return Ok(());
        };
        match source.client.indexing_progress().await {
            Ok(items) => {
                *consecutive_errors = 0;
                let progress = slice_catchup_progress(
                    &items,
                    source.relevant_bridges.as_deref(),
                    source.relevant_chains.as_ref(),
                );
                if progress.pairs_considered == 0 || progress.min_progress_ratio.is_none() {
                    tracing::warn!(
                        pairs_considered = progress.pairs_considered,
                        pairs_missing_progress = progress.pairs_missing_progress,
                        "the configured interchain filter selects no pair with usable catch-up \
                         progress; treating the interchain catch-up start check as satisfied"
                    );
                }
                let sufficient = is_catchup_progress_sufficient(
                    &progress,
                    &self.wait_config.interchain_catchup_min_progress,
                );
                let status = if sufficient {
                    InterchainIndexingStatus::CaughtUp
                } else {
                    InterchainIndexingStatus::CatchingUp
                };
                let modified = self.sender.send_if_modified(|val| {
                    if val.interchain != status {
                        val.interchain = status;
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
            Err(e) => {
                if *consecutive_errors >= RETRIES {
                    return Err(e).context("Requesting interchain indexing status");
                }
                tracing::warn!(
                    "Error ({consecutive_errors}/{RETRIES}) requesting interchain indexing status: {e:?}"
                );
                *consecutive_errors += 1;
            }
        }
        Ok(())
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        // Consult every enabled axis, not just blockscout and user ops. A missed
        // axis here leaves it at its seeded value forever and blocks every group
        // that depends on it. That is not a hypothetical: today this line ignores
        // `zetachain_checks_enabled()`, so a Zetachain-mode config with both
        // blockscout ratios and the user-ops check disabled returns here and
        // leaves the zetachain axis at `CatchingUp` for the life of the process.
        // Fixing that is a deliberate, in-scope side effect of this change —
        // mention it in the PR description.
        let blockscout_live = self.api_config.is_some();
        let any_check_live = (blockscout_live
            && (self.wait_config.blockscout_checks_enabled()
                || self.wait_config.user_ops_checks_enabled()))
            || self.wait_config.zetachain_checks_enabled()
            || self.wait_config.interchain_checks_enabled();
        if !any_check_live {
            tracing::info!("All indexing status checks are disabled, stopping status checks");
            return Ok(());
        }
        let mut blockscout_consecutive_errors = 0;
        let mut interchain_consecutive_errors = 0;
        loop {
            if blockscout_live && self.wait_config.blockscout_checks_enabled() {
                self.check_blockscout_status(&mut blockscout_consecutive_errors)
                    .await?;
            }
            if blockscout_live && self.wait_config.user_ops_checks_enabled() {
                self.check_user_ops_status().await;
            }
            if self.wait_config.zetachain_checks_enabled() {
                self.check_zetachain_status().await;
            }
            if self.wait_config.interchain_checks_enabled() {
                self.check_interchain_status(&mut interchain_consecutive_errors)
                    .await?;
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
    api_config: Option<blockscout_client::Configuration>,
    wait_config: StartConditionSettings,
    zetachain_cctx_db: Option<Arc<DatabaseConnection>>,
    interchain_api: Option<InterchainCatchupSource>,
) -> (IndexingStatusAggregator, IndexingStatusListener) {
    // enable immediately if the checks are disabled.
    // this allows to ignore the disabled checks when determining
    // whether any further checks are needed.
    //
    // `api_config.is_none()` counts as disabled: with no API there is no check
    // that can ever run, and seeding "not ready" would block every chart
    // forever. This is not hypothetical — before `init` was always called,
    // `IGNORE_BLOCKSCOUT_API_ABSENCE=true` with no URL produced no aggregator at
    // all, so nothing blocked. Removing the `.map()` in `server.rs` would
    // otherwise turn that configuration into a hang.
    let blockscout_checks_live = api_config.is_some();
    let blockscout_init_value = match (
        blockscout_checks_live && wait_config.blocks_ratio.enabled,
        blockscout_checks_live && wait_config.internal_transactions_ratio.enabled,
    ) {
        (true, _) => BlockscoutIndexingStatus::NoneIndexed,
        (false, true) => BlockscoutIndexingStatus::BlocksIndexed,
        (false, false) => BlockscoutIndexingStatus::InternalTransactionsIndexed,
    };
    let user_ops_init_value =
        if blockscout_checks_live && wait_config.user_ops_past_indexing_finished.enabled {
            UserOpsIndexingStatus::IndexingPastOperations
        } else {
            UserOpsIndexingStatus::PastOperationsIndexed
        };
    // unchanged: its "check enabled, DB missing" hole is a real bug
    // (.memory-bank/gotchas.md:546-555) but fixing it is a Zetachain-mode
    // behaviour change and is explicitly out of scope for this task.
    let zetachain_cctx_init_value = if wait_config.zetachain_checks_enabled() {
        ZetachainCctxIndexingStatus::CatchingUp
    } else {
        ZetachainCctxIndexingStatus::IndexedHistoricalData
    };
    // no `&& interchain_api.is_some()` clause is needed: work item 14's startup
    // `bail!` makes "check enabled without a URL" unreachable.
    let interchain_init_value = if wait_config.interchain_checks_enabled() {
        InterchainIndexingStatus::CatchingUp
    } else {
        InterchainIndexingStatus::CaughtUp
    };

    let (sender, receiver) = watch::channel(IndexingStatus {
        blockscout: blockscout_init_value,
        user_ops: user_ops_init_value,
        zetachain_cctx: zetachain_cctx_init_value,
        interchain: interchain_init_value,
    });
    (
        IndexingStatusAggregator {
            api_config,
            zetachain_cctx_db,
            interchain_api,
            wait_config,
            sender,
        },
        IndexingStatusListener { receiver },
    )
}

pub fn init_blockscout_api_client(
    settings: &Settings,
) -> anyhow::Result<Option<blockscout_client::Configuration>> {
    match (
        settings.ignore_blockscout_api_absence,
        &settings.blockscout_api_url,
    ) {
        (_, Some(blockscout_api_url)) => Ok(Some(blockscout_client::Configuration::new(
            blockscout_api_url.clone(),
        ))),
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

    use chrono::Utc;
    use rstest::*;
    use stats::tests::{
        mock_blockscout::{mock_blockscout_api, user_ops_status_response_json},
        mock_zetachain_cctx::init_imdb_with_watermark,
    };
    use std::time::Duration;
    use tokio::{select, task::JoinSet, time::error::Elapsed};
    use url::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use crate::settings::{ToggleableCheck, ToggleableOptionalCheck};

    use super::*;

    #[derive(Clone, Copy)]
    enum ZetachainTestInit {
        NoDb,
        /// Database won't have necessary tables, thus
        /// query will result in error.
        ErrorDb,
        DbNoTimestamp,
        DbOldTimestamp,
        DbTodayTimestamp,
    }

    impl ZetachainTestInit {
        async fn init_db(&self) -> Option<Arc<DatabaseConnection>> {
            let db = match self {
                ZetachainTestInit::NoDb => return None,
                ZetachainTestInit::ErrorDb => {
                    sea_orm::Database::connect("sqlite::memory:").await.unwrap()
                }
                ZetachainTestInit::DbNoTimestamp => init_imdb_with_watermark(None).await,
                ZetachainTestInit::DbOldTimestamp => {
                    init_imdb_with_watermark(Some(Utc::now() - chrono::Duration::days(2))).await
                }
                ZetachainTestInit::DbTodayTimestamp => {
                    init_imdb_with_watermark(Some(Utc::now())).await
                }
            };
            Some(Arc::new(db))
        }
    }

    /// As [`test_aggregator`], with the interchain source under the caller's
    /// control — for the interchain-axis tests, which need a real
    /// [`InterchainCatchupSource`] rather than the `None` every pre-existing
    /// call site is happy with.
    async fn test_aggregator_full(
        wait_config: StartConditionSettings,
        expected_status: IndexingStatus,
        timeout: Option<Duration>,
        response_blockscout: ResponseTemplate,
        response_user_ops: Option<ResponseTemplate>,
        zetachain_init: ZetachainTestInit,
        interchain_api: Option<InterchainCatchupSource>,
    ) -> Result<Result<(), anyhow::Error>, Elapsed> {
        let timeout = timeout.unwrap_or(Duration::from_millis(2000));
        let server = mock_blockscout_api(response_blockscout, response_user_ops).await;
        let db = zetachain_init.init_db().await;
        let api_config =
            blockscout_client::Configuration::new(Url::from_str(&server.uri()).unwrap());
        let (aggregator, mut listener) = init(Some(api_config), wait_config, db, interchain_api);
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

    async fn test_aggregator(
        wait_config: StartConditionSettings,
        expected_status: IndexingStatus,
        timeout: Option<Duration>,
        response_blockscout: ResponseTemplate,
        response_user_ops: Option<ResponseTemplate>,
        zetachain_init: ZetachainTestInit,
    ) -> Result<Result<(), anyhow::Error>, Elapsed> {
        test_aggregator_full(
            wait_config,
            expected_status,
            timeout,
            response_blockscout,
            response_user_ops,
            zetachain_init,
            None,
        )
        .await
    }

    #[fixture]
    fn wait_config(
        #[default(0.9)] blocks: f64,
        #[default(0.9)] internal_transactions: f64,
        #[default(true)] user_ops_check_enabled: bool,
        #[default(true)] zetachain_check_enabled: bool,
        #[default(false)] interchain_check_enabled: bool,
        #[default(0)] check_period_secs: u32,
    ) -> StartConditionSettings {
        StartConditionSettings {
            blocks_ratio: ToggleableThreshold::enabled(blocks),
            internal_transactions_ratio: ToggleableThreshold::enabled(internal_transactions),
            user_ops_past_indexing_finished: ToggleableCheck {
                enabled: user_ops_check_enabled,
            },
            zetachain_indexed_until_today: ToggleableOptionalCheck {
                enabled: Some(zetachain_check_enabled),
            },
            interchain_catchup_min_progress: ToggleableThreshold {
                enabled: interchain_check_enabled,
                threshold: 0.98,
            },
            check_period_secs,
        }
    }

    #[rstest]
    #[case(
        IndexingStatus {
            blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
            user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
            zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
            interchain: InterchainIndexingStatus::CaughtUp,
        },
        serde_json::json!({
            "finished_indexing": true,
            "finished_indexing_blocks": true,
            "indexed_blocks_ratio": "1.00",
            "indexed_internal_transactions_ratio": "1"
        }),
        Some(user_ops_status_response_json(true)),
        ZetachainTestInit::DbTodayTimestamp,
        false, // must succeed
    )]
    #[case(
        IndexingStatus::LEAST_RESTRICTIVE
            .with_blockscout(BlockscoutIndexingStatus::InternalTransactionsIndexed),
        serde_json::json!({
            "finished_indexing": false,
            "finished_indexing_blocks": false,
            "indexed_blocks_ratio": "0.80",
            "indexed_internal_transactions_ratio": "0.80"
        }),
        None,
        ZetachainTestInit::DbNoTimestamp,
        true, // must timeout
    )]
    #[case(
        IndexingStatus::LEAST_RESTRICTIVE
            .with_blockscout(BlockscoutIndexingStatus::InternalTransactionsIndexed),
        serde_json::json!({
            "finished_indexing": false,
            "finished_indexing_blocks": true,
            "indexed_blocks_ratio": "0.80",
            "indexed_internal_transactions_ratio": "1.00"
        }),
        None,
        ZetachainTestInit::DbOldTimestamp,
        true
    )]
    #[case(
        IndexingStatus::LEAST_RESTRICTIVE
            .with_blockscout(BlockscoutIndexingStatus::InternalTransactionsIndexed),
        serde_json::json!({
            "finished_indexing": true,
            "finished_indexing_blocks": false,
            "indexed_blocks_ratio": "1.00",
            "indexed_internal_transactions_ratio": "0.80"
        }),
        None,
        ZetachainTestInit::DbOldTimestamp,
        true
    )]
    #[case(
        IndexingStatus::LEAST_RESTRICTIVE
            .with_blockscout(BlockscoutIndexingStatus::InternalTransactionsIndexed)
            .with_user_ops(UserOpsIndexingStatus::PastOperationsIndexed),
        serde_json::json!({
            "finished_indexing": true,
            "finished_indexing_blocks": true,
            "indexed_blocks_ratio": "1.00",
            "indexed_internal_transactions_ratio": "1.00"
        }),
        Some(user_ops_status_response_json(false)),
        ZetachainTestInit::DbOldTimestamp,
        true
    )]
    #[case(
        IndexingStatus::LEAST_RESTRICTIVE
            .with_blockscout(BlockscoutIndexingStatus::BlocksIndexed)
            .with_user_ops(UserOpsIndexingStatus::PastOperationsIndexed),
        serde_json::json!({
            "finished_indexing": true,
            "finished_indexing_blocks": false,
            "indexed_blocks_ratio": "1.00",
            "indexed_internal_transactions_ratio": "0.80"
        }),
        Some(user_ops_status_response_json(true)),
        ZetachainTestInit::DbOldTimestamp,
        false, // must succeed
    )]
    #[case(
        IndexingStatus::LEAST_RESTRICTIVE
            .with_zetachain_cctx(ZetachainCctxIndexingStatus::IndexedHistoricalData),
        serde_json::json!({
            "finished_indexing": true,
            "finished_indexing_blocks": true,
            "indexed_blocks_ratio": "1.00",
            "indexed_internal_transactions_ratio": "1.00"
        }),
        Some(user_ops_status_response_json(true)),
        ZetachainTestInit::DbOldTimestamp,
        true
    )]
    #[case(
        IndexingStatus::LEAST_RESTRICTIVE
            .with_zetachain_cctx(ZetachainCctxIndexingStatus::IndexedHistoricalData),
        serde_json::json!({
            "finished_indexing": true,
            "finished_indexing_blocks": true,
            "indexed_blocks_ratio": "1.00",
            "indexed_internal_transactions_ratio": "1.00"
        }),
        Some(user_ops_status_response_json(true)),
        ZetachainTestInit::NoDb,
        true
    )]
    #[case(
        IndexingStatus::LEAST_RESTRICTIVE
            .with_zetachain_cctx(ZetachainCctxIndexingStatus::IndexedHistoricalData),
        serde_json::json!({
            "finished_indexing": true,
            "finished_indexing_blocks": true,
            "indexed_blocks_ratio": "1.00",
            "indexed_internal_transactions_ratio": "1.00"
        }),
        Some(user_ops_status_response_json(true)),
        ZetachainTestInit::DbTodayTimestamp,
        false
    )]
    #[tokio::test]
    async fn waiter_works_with_200_response(
        wait_config: StartConditionSettings,
        #[case] expected_status: IndexingStatus,
        #[case] response_blockscout: serde_json::Value,
        #[case] response_user_ops: Option<serde_json::Value>,
        #[case] zetachain_init: ZetachainTestInit,
        #[case] must_timeout: bool,
    ) {
        let result = test_aggregator(
            wait_config.clone(),
            expected_status,
            None,
            ResponseTemplate::new(200).set_body_json(response_blockscout),
            response_user_ops.map(|r| ResponseTemplate::new(200).set_body_json(r)),
            zetachain_init,
        )
        .await;
        if must_timeout {
            result.expect_err("must time out");
        } else {
            result.expect("must not timeout").expect("must not error");
        }
    }

    #[rstest]
    #[tokio::test]
    async fn waiter_works_with_slow_response(wait_config: StartConditionSettings) {
        test_aggregator(
            wait_config,
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
                interchain: InterchainIndexingStatus::CaughtUp,
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
            ZetachainTestInit::DbTodayTimestamp,
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
                zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
                interchain: InterchainIndexingStatus::CaughtUp,
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
            ZetachainTestInit::DbTodayTimestamp,
        )
        .await
        .expect_err("must time out");

        test_aggregator(
            wait_config.clone(),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
                interchain: InterchainIndexingStatus::CaughtUp,
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
            ZetachainTestInit::DbTodayTimestamp,
        )
        .await
        .expect_err("must time out");

        test_aggregator(
            wait_config,
            IndexingStatus::LEAST_RESTRICTIVE,
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
            ZetachainTestInit::NoDb,
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
            IndexingStatus::LEAST_RESTRICTIVE
                .with_blockscout(BlockscoutIndexingStatus::BlocksIndexed),
            Some(Duration::from_millis(500)),
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "finished_indexing": false,
                "finished_indexing_blocks": false,
                "indexed_blocks_ratio": null,
                "indexed_internal_transactions_ratio": null
            })),
            None,
            ZetachainTestInit::NoDb,
        )
        .await
        .expect_err("must time out and not fall with error");
    }

    #[rstest]
    #[tokio::test]
    async fn waiter_retries_with_error_codes(
        #[with(0.9, 0.9, true, true, false, 1)] wait_config: StartConditionSettings,
    ) {
        let timeout = Some(Duration::from_millis(1500));
        let s = IndexingStatus::LEAST_RESTRICTIVE
            .with_blockscout(BlockscoutIndexingStatus::BlocksIndexed);
        let r = |code: u16| ResponseTemplate::new(code);
        let n = ZetachainTestInit::NoDb;
        let mut error_servers = JoinSet::from_iter([
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(429), None, n),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(500), None, n),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(503), None, n),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(504), None, n),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(400), None, n),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(403), None, n),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(404), None, n),
            test_aggregator(wait_config.clone(), s.clone(), timeout, r(405), None, n),
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
        let ok_z = ZetachainTestInit::DbTodayTimestamp;
        let err_z = ZetachainTestInit::ErrorDb;
        let config_b_off = StartConditionSettings {
            blocks_ratio: ToggleableThreshold::disabled(),
            internal_transactions_ratio: ToggleableThreshold::disabled(),
            user_ops_past_indexing_finished: ToggleableCheck { enabled: true },
            zetachain_indexed_until_today: ToggleableOptionalCheck {
                enabled: Some(true),
            },
            interchain_catchup_min_progress: ToggleableThreshold::disabled().set_threshold(0.98),
            check_period_secs: 1,
        };
        let config_u_off = StartConditionSettings {
            blocks_ratio: ToggleableThreshold::default(),
            internal_transactions_ratio: ToggleableThreshold::default(),
            user_ops_past_indexing_finished: ToggleableCheck { enabled: false },
            zetachain_indexed_until_today: ToggleableOptionalCheck {
                enabled: Some(true),
            },
            interchain_catchup_min_progress: ToggleableThreshold::disabled().set_threshold(0.98),
            check_period_secs: 1,
        };
        let config_z_off = StartConditionSettings {
            blocks_ratio: ToggleableThreshold::default(),
            internal_transactions_ratio: ToggleableThreshold::default(),
            user_ops_past_indexing_finished: ToggleableCheck { enabled: true },
            zetachain_indexed_until_today: ToggleableOptionalCheck {
                enabled: Some(false),
            },
            interchain_catchup_min_progress: ToggleableThreshold::disabled().set_threshold(0.98),
            check_period_secs: 1,
        };
        let mut tests = JoinSet::from_iter(
            [
                (&config_b_off, &s, timeout, &r(400), Some(&ok_u), ok_z),
                (&config_b_off, &s, timeout, &r(404), Some(&ok_u), ok_z),
                (&config_u_off, &s, timeout, &ok_b, Some(&r(400)), ok_z),
                (&config_u_off, &s, timeout, &ok_b, Some(&r(404)), ok_z),
                (&config_z_off, &s, timeout, &ok_b, Some(&ok_u), err_z),
                (&config_z_off, &s, timeout, &ok_b, Some(&ok_u), err_z),
            ]
            .map(|(a, b, c, d, e, f)| {
                test_aggregator(a.clone(), b.clone(), c, d.clone(), e.cloned(), f)
            }),
        );
        while let Some(server) = tests.join_next().await {
            let test_result = server.unwrap();
            test_result
                .expect("must not timeout")
                .expect("must not error");
        }
    }

    /// One `/api/v1/status/indexing` row, matching the interchain indexer's
    /// wire shape (`chain_id` a JSON string, `catchup_progress_percent` a plain
    /// number).
    fn progress_item(
        bridge_id: i32,
        chain_id: i64,
        catchup_complete: bool,
        catchup_progress_percent: Option<f64>,
    ) -> serde_json::Value {
        serde_json::json!({
            "bridge_id": bridge_id,
            "chain_id": chain_id.to_string(),
            "catchup_complete": catchup_complete,
            "catchup_progress_percent": catchup_progress_percent,
        })
    }

    /// An unbounded [`InterchainCatchupSource`] pointed at `uri`.
    fn interchain_catchup_source(uri: &str) -> InterchainCatchupSource {
        let url = Url::from_str(uri).unwrap();
        let client = InterchainIndexerApiClient::try_new(Some(&url))
            .unwrap()
            .expect("a URL was provided");
        InterchainCatchupSource {
            client: Arc::new(client),
            relevant_bridges: None,
            relevant_chains: None,
        }
    }

    fn interchain_disabled_wait_config() -> StartConditionSettings {
        StartConditionSettings {
            blocks_ratio: ToggleableThreshold::disabled(),
            internal_transactions_ratio: ToggleableThreshold::disabled(),
            user_ops_past_indexing_finished: ToggleableCheck { enabled: false },
            zetachain_indexed_until_today: ToggleableOptionalCheck {
                enabled: Some(false),
            },
            interchain_catchup_min_progress: ToggleableThreshold::disabled().set_threshold(0.98),
            check_period_secs: 0,
        }
    }

    fn interchain_only_wait_config(threshold: f64) -> StartConditionSettings {
        StartConditionSettings {
            interchain_catchup_min_progress: ToggleableThreshold::enabled(threshold),
            ..interchain_disabled_wait_config()
        }
    }

    #[test]
    fn init_seeds_the_interchain_axis_satisfied_when_the_check_is_disabled() {
        let (_aggregator, listener) = init(None, interchain_disabled_wait_config(), None, None);
        assert!(
            listener.receiver.borrow().is_requirement_satisfied(
                &IndexingStatus::LEAST_RESTRICTIVE
                    .with_interchain(InterchainIndexingStatus::CaughtUp)
            ),
            "a freshly init'ed listener must already satisfy the interchain requirement when \
             the check is disabled, with no poll at all"
        );
    }

    #[test]
    fn init_seeds_the_interchain_axis_catching_up_when_the_check_is_enabled() {
        let (_aggregator, listener) = init(None, interchain_only_wait_config(0.98), None, None);
        assert!(
            !listener.receiver.borrow().is_requirement_satisfied(
                &IndexingStatus::LEAST_RESTRICTIVE
                    .with_interchain(InterchainIndexingStatus::CaughtUp)
            ),
            "the converse of the disabled case, at t=0"
        );
    }

    /// The regression guard for work item 19b: `init` is now always called,
    /// so a deployment with `STATS__IGNORE_BLOCKSCOUT_API_ABSENCE=true` and no
    /// URL must still seed the blockscout axis satisfied, even though
    /// `blocks_ratio` defaults to *enabled*. Without this, every chart in every
    /// such deployment blocks forever.
    #[test]
    fn init_seeds_blockscout_axes_satisfied_when_no_api_config() {
        let (_aggregator, listener) = init(None, StartConditionSettings::default(), None, None);
        assert!(
            listener.receiver.borrow().is_requirement_satisfied(
                &IndexingStatus::LEAST_RESTRICTIVE
                    .with_blockscout(BlockscoutIndexingStatus::BlocksIndexed)
            ),
            "with no api_config, the blockscout axis must seed satisfied regardless of \
             blocks_ratio's enabled-by-default value"
        );
    }

    #[tokio::test]
    async fn run_stops_when_only_disabled_checks_remain() {
        let (aggregator, _listener) = init(None, interchain_disabled_wait_config(), None, None);
        tokio::time::timeout(Duration::from_millis(500), aggregator.run())
            .await
            .expect("run must return promptly when every check is disabled")
            .expect("run must not error");
    }

    /// The pre-existing latent zetachain bug, asserted: on today's (unfixed)
    /// `run`, both blockscout ratios and the user-ops check disabled makes
    /// `run` return before the loop even though the zetachain check is
    /// enabled, and this test times out.
    #[tokio::test]
    async fn run_keeps_checking_when_only_the_zetachain_check_is_enabled() {
        let wait_config = StartConditionSettings {
            zetachain_indexed_until_today: ToggleableOptionalCheck {
                enabled: Some(true),
            },
            ..interchain_disabled_wait_config()
        };
        let db = Arc::new(init_imdb_with_watermark(Some(Utc::now())).await);
        let (aggregator, mut listener) = init(None, wait_config, Some(db), None);
        let target = IndexingStatus::LEAST_RESTRICTIVE
            .with_zetachain_cctx(ZetachainCctxIndexingStatus::IndexedHistoricalData);
        select! {
            res = aggregator.run() => panic!("aggregator terminated with error: {res:?}"),
            result = tokio::time::timeout(Duration::from_millis(1000), listener.wait_until_status_at_least(target)) => {
                result
                    .expect("must not time out")
                    .expect("channel must not close");
            }
        }
    }

    /// The interchain twin of the zetachain test above.
    #[tokio::test]
    async fn run_keeps_checking_when_only_the_interchain_check_is_enabled() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/status/indexing"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [progress_item(1, 10, true, Some(100.0))]
            })))
            .mount(&server)
            .await;
        let wait_config = interchain_only_wait_config(0.98);
        let (aggregator, mut listener) = init(
            None,
            wait_config,
            None,
            Some(interchain_catchup_source(&server.uri())),
        );
        let target =
            IndexingStatus::LEAST_RESTRICTIVE.with_interchain(InterchainIndexingStatus::CaughtUp);
        select! {
            res = aggregator.run() => panic!("aggregator terminated with error: {res:?}"),
            result = tokio::time::timeout(Duration::from_millis(1000), listener.wait_until_status_at_least(target)) => {
                result
                    .expect("must not time out")
                    .expect("channel must not close");
            }
        }
    }

    /// "Groups wait, then start", at the axis level.
    #[tokio::test]
    async fn interchain_check_publishes_catching_up_below_the_threshold() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/status/indexing"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [progress_item(1, 10, false, Some(10.0))]
            })))
            .mount(&server)
            .await;
        let wait_config = interchain_only_wait_config(0.98);
        let (aggregator, mut listener) = init(
            None,
            wait_config,
            None,
            Some(interchain_catchup_source(&server.uri())),
        );
        let target =
            IndexingStatus::LEAST_RESTRICTIVE.with_interchain(InterchainIndexingStatus::CaughtUp);

        let aggregator_future = aggregator.run();
        let mut probe = listener.clone();
        select! {
            res = aggregator_future => panic!("aggregator terminated with error: {res:?}"),
            result = tokio::time::timeout(Duration::from_millis(500), probe.wait_until_status_at_least(target.clone())) => {
                assert!(result.is_err(), "must not reach CaughtUp while progress is below the threshold");
            }
        }

        server.reset().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/status/indexing"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [progress_item(1, 10, true, Some(100.0))]
            })))
            .mount(&server)
            .await;

        select! {
            res = aggregator.run() => panic!("aggregator terminated with error: {res:?}"),
            result = tokio::time::timeout(Duration::from_millis(1000), listener.wait_until_status_at_least(target)) => {
                result
                    .expect("must reach CaughtUp once the mock reports full progress")
                    .expect("channel must not close");
            }
        }
    }

    /// Pins "unknown ⇒ do not block" for a *successful* call: neither a
    /// relevant pair with no reported percentage nor an empty relevant set
    /// should hold the start check back.
    #[tokio::test]
    async fn interchain_check_treats_unknown_progress_as_satisfied() {
        for items in [
            serde_json::json!({"items": [progress_item(1, 10, false, None)]}),
            serde_json::json!({"items": []}),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/api/v1/status/indexing"))
                .respond_with(ResponseTemplate::new(200).set_body_json(items.clone()))
                .mount(&server)
                .await;
            let (aggregator, listener) = init(
                None,
                interchain_only_wait_config(0.98),
                None,
                Some(interchain_catchup_source(&server.uri())),
            );
            let mut consecutive_errors = 0;
            aggregator
                .check_interchain_status(&mut consecutive_errors)
                .await
                .expect("a successful call with no usable progress must not error");
            assert!(
                listener.receiver.borrow().is_requirement_satisfied(
                    &IndexingStatus::LEAST_RESTRICTIVE
                        .with_interchain(InterchainIndexingStatus::CaughtUp)
                ),
                "unknown progress must resolve to CaughtUp (do not block): {items}"
            );
        }
    }

    /// Pins the chosen policy against the `check_zetachain_status`
    /// anti-pattern in both halves: it does not silently pass, and it does not
    /// hang forever.
    #[tokio::test]
    async fn interchain_check_retries_then_fails_like_the_blockscout_check() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/status/indexing"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let (aggregator, listener) = init(
            None,
            interchain_only_wait_config(0.98),
            None,
            Some(interchain_catchup_source(&server.uri())),
        );
        let run_result = tokio::time::timeout(Duration::from_secs(5), aggregator.run())
            .await
            .expect("run must not hang forever");
        assert!(
            run_result.is_err(),
            "run must fail after RETRIES consecutive errors, not hang or silently pass"
        );
        assert_eq!(
            listener.receiver.borrow().interchain,
            InterchainIndexingStatus::CatchingUp,
            "the axis must keep its last known (seeded) value on error, not flip to CaughtUp"
        );
    }
}
