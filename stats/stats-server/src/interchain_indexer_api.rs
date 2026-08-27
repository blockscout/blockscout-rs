// SPDX-License-Identifier: LicenseRef-Blockscout

//! Reading the interchain indexer's catch-up status over its HTTP API.
//!
//! Consumed as plain JSON on purpose: `interchain-indexer-proto` is not a stats
//! dependency and must not become one.

use std::{collections::BTreeSet, time::Duration};

use reqwest::StatusCode;
use thiserror::Error;
use url::{ParseError, Url};

use crate::settings::ToggleableThreshold;

pub const INDEXING_STATUS_PATH: &str = "api/v1/status/indexing";
const TIMEOUT: Duration = Duration::from_secs(5);
/// Total attempts per cycle, not retries-after-the-first.
const ATTEMPTS: u32 = 2;
const RETRY_BACKOFF: Duration = Duration::from_millis(250);

#[derive(Debug, Clone)]
pub struct InterchainIndexerApiClient {
    client: reqwest::Client,
    base_url: Url,
}

#[derive(Debug, Error)]
pub enum InterchainIndexerApiError {
    #[error("interchain indexer status request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("interchain indexer status returned unexpected status {0}")]
    UnexpectedStatus(StatusCode),
    #[error("failed to construct interchain indexer status URL: {0}")]
    InvalidUrl(#[from] ParseError),
}

/// One row of `GET /api/v1/status/indexing`, narrowed to what stats needs.
///
/// Wire-format facts, both from `interchain-indexer-proto/build.rs`: keys are
/// snake_case, and every `i64`/`u64` proto field is serialized as a JSON
/// **string** (`actix-prost-macros` injects `DisplayFromStr` for those two
/// types), so `chain_id` arrives as `"1"`. `bridge_id` is `i32` and stays a
/// number. Unknown fields are ignored deliberately — the payload carries six
/// more that stats does not read.
#[serde_with::serde_as]
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct ChainIndexingProgress {
    pub bridge_id: i32,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub chain_id: i64,
    pub catchup_complete: bool,
    /// Field 8 of the payload: the share of the configured block range that has
    /// been **scanned**, `0.0..=100.0`. Not a completeness measure — see
    /// `interchain-indexer-logic/src/indexer/progress.rs`'s module doc. `double`
    /// on the wire, so — unlike `chain_id` — a plain JSON number, not a string.
    ///
    /// `Option` with `#[serde(default)]` on purpose. The indexer always sends
    /// it, so this is defensive; the point is the failure direction. A hard parse
    /// error would break the **verdict** too, in a deployment that has the gate
    /// disabled and does not care about progress. `None` instead means "this pair
    /// reported no percentage" ⇒ progress unknown ⇒ the gate does not block, and
    /// the verdict is untouched. Deliberately **not** `#[serde(default)] f64`,
    /// which would read a missing field as `0.0` and block forever.
    #[serde(default)]
    pub catchup_progress_percent: Option<f64>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct GetIndexingProgressResponse {
    #[serde(default)]
    items: Vec<ChainIndexingProgress>,
}

impl InterchainIndexerApiClient {
    /// `None` when no URL is configured — the check is then disabled.
    pub fn try_new(base_url: Option<&Url>) -> Result<Option<Self>, reqwest::Error> {
        let Some(base_url) = base_url else {
            return Ok(None);
        };
        let base_url = normalize_base_url(base_url.clone());
        let client = reqwest::Client::builder().timeout(TIMEOUT).build()?;
        Ok(Some(Self { client, base_url }))
    }

    /// All pairs. The endpoint's `bridge_id` / `chain_id` query parameters are
    /// single-valued, so server-side narrowing would mean N requests; the
    /// payload is one small row per pair, so fetch everything and narrow in
    /// `slice_catchup_verdict`.
    pub async fn indexing_progress(
        &self,
    ) -> Result<Vec<ChainIndexingProgress>, InterchainIndexerApiError> {
        let url = self.base_url.join(INDEXING_STATUS_PATH)?;
        let mut attempt = 0;
        loop {
            attempt += 1;
            match self.client.get(url.clone()).send().await {
                Ok(response) => {
                    let status = response.status();
                    // A 5xx is presumed transient (the indexer's own request
                    // handling failing, not a malformed request on our side),
                    // so it gets the same retry budget as a transport error.
                    // A 4xx is not retried: retrying an unchanged request
                    // against an unchanged endpoint would just repeat it.
                    if status.is_server_error() && attempt < ATTEMPTS {
                        tokio::time::sleep(RETRY_BACKOFF).await;
                        continue;
                    }
                    if !status.is_success() {
                        return Err(InterchainIndexerApiError::UnexpectedStatus(status));
                    }
                    let body: GetIndexingProgressResponse = response.json().await?;
                    return Ok(body.items);
                }
                Err(err) if attempt < ATTEMPTS => {
                    tokio::time::sleep(RETRY_BACKOFF).await;
                    let _ = err;
                    continue;
                }
                Err(err) => return Err(InterchainIndexerApiError::Request(err)),
            }
        }
    }
}

fn normalize_base_url(mut base_url: Url) -> Url {
    let trimmed_path = base_url.path().trim_end_matches('/');
    let normalized_path = if trimmed_path.is_empty() {
        "/".to_string()
    } else {
        format!("{trimmed_path}/")
    };
    base_url.set_path(&normalized_path);
    base_url
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictSource {
    IndexerApi,
    NotConfigured,
    ApiUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceCatchupVerdict {
    pub complete: bool,
    pub pairs_considered: usize,
    /// Relevant pairs whose `catchup_complete` is `false`; empty when complete.
    /// Named in the `warn` so a permanently-stuck pair is diagnosable from logs.
    pub holding: Vec<(i32, i64)>,
    pub source: VerdictSource,
}

/// Whether `(bridge_id, chain_id)` falls inside the configured slice's scope.
/// `None` for either scope means "unbounded". Shared by [`slice_catchup_verdict`]
/// and [`slice_catchup_progress`] so both use one definition of "relevant".
fn is_relevant(
    bridge_id: i32,
    chain_id: i64,
    relevant_bridges: Option<&[i32]>,
    relevant_chains: Option<&BTreeSet<i64>>,
) -> bool {
    relevant_bridges.is_none_or(|bridges| bridges.contains(&bridge_id))
        && relevant_chains.is_none_or(|chains| chains.contains(&chain_id))
}

fn relevant_pairs<'a>(
    progress: &'a [ChainIndexingProgress],
    relevant_bridges: Option<&'a [i32]>,
    relevant_chains: Option<&'a BTreeSet<i64>>,
) -> impl Iterator<Item = &'a ChainIndexingProgress> {
    progress.iter().filter(move |row| {
        is_relevant(
            row.bridge_id,
            row.chain_id,
            relevant_bridges,
            relevant_chains,
        )
    })
}

/// Conjunction of `catchup_complete` over the pairs relevant to the configured
/// slice. `None` for either scope means "unbounded".
///
/// Vacuously `true` over an empty relevant set — correct (no relevant pair is
/// catching up), and the caller warns when that happens against a non-empty
/// payload, because it also means the filter selects pairs the indexer does not
/// index.
pub fn slice_catchup_verdict(
    progress: &[ChainIndexingProgress],
    relevant_bridges: Option<&[i32]>,
    relevant_chains: Option<&BTreeSet<i64>>,
) -> SliceCatchupVerdict {
    let relevant: Vec<&ChainIndexingProgress> =
        relevant_pairs(progress, relevant_bridges, relevant_chains).collect();
    let holding: Vec<(i32, i64)> = relevant
        .iter()
        .filter(|row| !row.catchup_complete)
        .map(|row| (row.bridge_id, row.chain_id))
        .collect();
    SliceCatchupVerdict {
        complete: holding.is_empty(),
        pairs_considered: relevant.len(),
        holding,
        source: VerdictSource::IndexerApi,
    }
}

/// The "unknown ⇒ do not force" resolution policy, in one place.
///
/// `response == None` means no client is configured. A failed or absent call
/// resolves to `complete = true`: the verdict is stateless and re-read every
/// cycle, so a wrong `true` costs delay and not data, and the case it would
/// otherwise lose is covered by the per-chart stored-floor check. Never "skip
/// the group" — an API outage must not freeze chart updates.
pub fn resolve_verdict(
    response: Option<Result<Vec<ChainIndexingProgress>, InterchainIndexerApiError>>,
    relevant_bridges: Option<&[i32]>,
    relevant_chains: Option<&BTreeSet<i64>>,
) -> SliceCatchupVerdict {
    match response {
        None => SliceCatchupVerdict {
            complete: true,
            pairs_considered: 0,
            holding: Vec::new(),
            source: VerdictSource::NotConfigured,
        },
        Some(Ok(items)) => slice_catchup_verdict(&items, relevant_bridges, relevant_chains),
        Some(Err(_)) => SliceCatchupVerdict {
            complete: true,
            pairs_considered: 0,
            holding: Vec::new(),
            source: VerdictSource::ApiUnavailable,
        },
    }
}

/// `Some(catching_up)` when the verdict is worth publishing, `None` when it must
/// leave the last known value alone.
///
/// Only a verdict actually **derived from the API** is published. An
/// unreachable API or an unset URL leaves the last known value in place,
/// mirroring `IndexingStatusAggregator`, which changes nothing on an API
/// error. Clobbering to `None` on every transient blip would make the field
/// flap to absent exactly when an operator is watching it.
pub fn catchup_state_to_publish(verdict: &SliceCatchupVerdict) -> Option<bool> {
    if verdict.source == VerdictSource::IndexerApi {
        Some(!verdict.complete)
    } else {
        None
    }
}

/// `min(catchup_progress_percent)` over the pairs relevant to the configured
/// slice, converted to a **ratio** so the setting reads exactly like
/// `STATS__CONDITIONAL_START__BLOCKS_RATIO__THRESHOLD`.
///
/// `min`, not mean or max: the same "slowest relevant pair decides" rule the
/// verdict expresses as a conjunction.
#[derive(Debug, Clone, PartialEq)]
pub struct SliceCatchupProgress {
    /// `None` = unknown, which never blocks. Unknown for two distinct reasons,
    /// separated by the counters below: no relevant pair at all, or a relevant
    /// pair that reported no percentage.
    pub min_progress_ratio: Option<f64>,
    pub pairs_considered: usize,
    pub pairs_missing_progress: usize,
    /// The pair that supplied the minimum. Named in the wait log so an operator
    /// can see which pair is holding the start.
    pub slowest: Option<(i32, i64)>,
}

/// Scope inputs are the same two the verdict takes; `None` means unbounded.
///
/// The unit conversion happens here, once: the API's `catchup_progress_percent`
/// is `0.0..=100.0`; this divides by `100.0` to produce the ratio the threshold
/// setting is expressed in. Getting it wrong is silent in both directions — a
/// threshold of `0.95` against a raw `95.0` always passes, and `95.0` against a
/// ratio `0.95` never does.
pub fn slice_catchup_progress(
    progress: &[ChainIndexingProgress],
    relevant_bridges: Option<&[i32]>,
    relevant_chains: Option<&BTreeSet<i64>>,
) -> SliceCatchupProgress {
    let relevant: Vec<&ChainIndexingProgress> =
        relevant_pairs(progress, relevant_bridges, relevant_chains).collect();
    let pairs_considered = relevant.len();
    let pairs_missing_progress = relevant
        .iter()
        .filter(|row| row.catchup_progress_percent.is_none())
        .count();
    let slowest_row = relevant
        .into_iter()
        .filter_map(|row| row.catchup_progress_percent.map(|percent| (row, percent)))
        .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let (min_progress_ratio, slowest) = match slowest_row {
        Some((row, percent)) => (Some(percent / 100.0), Some((row.bridge_id, row.chain_id))),
        None => (None, None),
    };
    SliceCatchupProgress {
        min_progress_ratio,
        pairs_considered,
        pairs_missing_progress,
        slowest,
    }
}

/// `true` ⇒ do not block. A disabled threshold passes, and so does an unknown
/// minimum: "unknown ⇒ do not block", the same direction as the verdict's
/// unknown ⇒ `true`.
pub fn is_catchup_progress_sufficient(
    progress: &SliceCatchupProgress,
    threshold: &ToggleableThreshold,
) -> bool {
    if !threshold.enabled {
        return true;
    }
    match progress.min_progress_ratio {
        Some(ratio) => ratio >= threshold.threshold,
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use super::*;
    use crate::settings::ToggleableThreshold;

    fn row(bridge_id: i32, chain_id: i64, catchup_complete: bool) -> ChainIndexingProgress {
        ChainIndexingProgress {
            bridge_id,
            chain_id,
            catchup_complete,
            catchup_progress_percent: None,
        }
    }

    fn row_with_progress(
        bridge_id: i32,
        chain_id: i64,
        catchup_complete: bool,
        percent: Option<f64>,
    ) -> ChainIndexingProgress {
        ChainIndexingProgress {
            bridge_id,
            chain_id,
            catchup_complete,
            catchup_progress_percent: percent,
        }
    }

    #[test]
    fn verdict_is_the_conjunction_over_relevant_pairs() {
        let progress = vec![row(1, 10, true), row(1, 11, true)];
        let verdict = slice_catchup_verdict(&progress, None, None);
        assert!(verdict.complete);
        assert_eq!(verdict.pairs_considered, 2);

        let progress = vec![row(1, 10, true), row(1, 11, false)];
        let verdict = slice_catchup_verdict(&progress, None, None);
        assert!(!verdict.complete);
        assert_eq!(verdict.pairs_considered, 2);
    }

    #[test]
    fn verdict_ignores_a_catching_up_bridge_outside_bridge_ids() {
        let progress = vec![row(1, 10, true), row(2, 10, false)];
        let verdict = slice_catchup_verdict(&progress, Some(&[1]), None);
        assert!(verdict.complete);
        assert_eq!(verdict.pairs_considered, 1);
    }

    #[test]
    fn verdict_ignores_a_catching_up_chain_outside_relevant_chains() {
        let progress = vec![row(1, 10, true), row(1, 99, false)];
        let relevant_chains: BTreeSet<i64> = [10].into_iter().collect();
        let verdict = slice_catchup_verdict(&progress, None, Some(&relevant_chains));
        assert!(verdict.complete);
        assert_eq!(verdict.pairs_considered, 1);
    }

    #[test]
    fn verdict_names_the_pairs_holding_it_false() {
        let progress = vec![row(1, 10, false), row(2, 20, false), row(3, 30, true)];
        let verdict = slice_catchup_verdict(&progress, None, None);
        assert!(!verdict.complete);
        assert_eq!(verdict.holding, vec![(1, 10), (2, 20)]);
    }

    #[test]
    fn verdict_over_no_relevant_pairs_is_vacuously_complete() {
        let progress = vec![row(1, 10, false)];
        let verdict = slice_catchup_verdict(&progress, Some(&[99]), None);
        assert!(verdict.complete);
        assert_eq!(verdict.pairs_considered, 0);
    }

    #[test]
    fn verdict_defaults_to_complete_when_the_api_is_unreachable() {
        let verdict = resolve_verdict(
            Some(Err(InterchainIndexerApiError::UnexpectedStatus(
                StatusCode::INTERNAL_SERVER_ERROR,
            ))),
            None,
            None,
        );
        assert!(verdict.complete);
        assert_eq!(verdict.source, VerdictSource::ApiUnavailable);
    }

    #[test]
    fn verdict_defaults_to_complete_when_no_api_url_is_configured() {
        let verdict = resolve_verdict(None, None, None);
        assert!(verdict.complete);
        assert_eq!(verdict.source, VerdictSource::NotConfigured);
    }

    #[test]
    fn progress_row_deserializes_string_encoded_chain_id() {
        let json = r#"{
            "bridge_id": 1,
            "chain_id": "100",
            "catchup_complete": true,
            "catchup_progress_percent": 95.5,
            "some_other_field": "ignored",
            "yet_another": 42
        }"#;
        let parsed: ChainIndexingProgress = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed,
            row_with_progress(1, 100, true, Some(95.5)),
            "chain_id must parse from a JSON string, and catchup_progress_percent from a \
             plain JSON number"
        );
    }

    #[tokio::test]
    async fn indexing_progress_parses_the_indexer_payload() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/{INDEXING_STATUS_PATH}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [
                    {
                        "bridge_id": 1,
                        "chain_id": "10",
                        "catchup_complete": true,
                        "catchup_progress_percent": 100.0
                    },
                    {
                        "bridge_id": 1,
                        "chain_id": "20",
                        "catchup_complete": false,
                        "catchup_progress_percent": 40.0
                    }
                ]
            })))
            .mount(&server)
            .await;
        let url = Url::parse(&server.uri()).unwrap();
        let client = InterchainIndexerApiClient::try_new(Some(&url))
            .unwrap()
            .unwrap();
        let items = client.indexing_progress().await.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].chain_id, 10);
        assert!(items[0].catchup_complete);
        assert_eq!(items[1].chain_id, 20);
        assert!(!items[1].catchup_complete);
    }

    #[tokio::test]
    async fn indexing_progress_reports_a_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/{INDEXING_STATUS_PATH}")))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let url = Url::parse(&server.uri()).unwrap();
        let client = InterchainIndexerApiClient::try_new(Some(&url))
            .unwrap()
            .unwrap();
        let err = client.indexing_progress().await.unwrap_err();
        assert!(matches!(
            err,
            InterchainIndexerApiError::UnexpectedStatus(_)
        ));
    }

    /// A 5xx gets the same `ATTEMPTS` retry budget as a transport error — only
    /// the second (final) attempt's failure is returned. `.expect(2)` is
    /// checked when `server` drops at the end of the test; if the request were
    /// only made once (the pre-fix behaviour), this test panics on drop rather
    /// than merely returning the same error the un-retried version would.
    #[tokio::test]
    async fn indexing_progress_retries_a_5xx_before_failing() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/{INDEXING_STATUS_PATH}")))
            .respond_with(ResponseTemplate::new(503))
            .expect(2)
            .mount(&server)
            .await;
        let url = Url::parse(&server.uri()).unwrap();
        let client = InterchainIndexerApiClient::try_new(Some(&url))
            .unwrap()
            .unwrap();
        let err = client.indexing_progress().await.unwrap_err();
        assert!(matches!(
            err,
            InterchainIndexerApiError::UnexpectedStatus(_)
        ));
    }

    /// A 4xx is not retried: `.expect(1)` fails the test on drop if a second
    /// request was made.
    #[tokio::test]
    async fn indexing_progress_does_not_retry_a_4xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/{INDEXING_STATUS_PATH}")))
            .respond_with(ResponseTemplate::new(404))
            .expect(1)
            .mount(&server)
            .await;
        let url = Url::parse(&server.uri()).unwrap();
        let client = InterchainIndexerApiClient::try_new(Some(&url))
            .unwrap()
            .unwrap();
        let err = client.indexing_progress().await.unwrap_err();
        assert!(matches!(
            err,
            InterchainIndexerApiError::UnexpectedStatus(_)
        ));
    }

    #[test]
    fn catchup_progress_is_the_min_over_relevant_pairs() {
        let progress = vec![
            row_with_progress(1, 10, true, Some(100.0)),
            row_with_progress(1, 11, false, Some(40.0)),
            row_with_progress(1, 12, false, Some(90.0)),
        ];
        let result = slice_catchup_progress(&progress, None, None);
        assert_eq!(result.min_progress_ratio, Some(0.40));
        assert_eq!(result.slowest, Some((1, 11)));
        assert_eq!(result.pairs_considered, 3);
    }

    #[test]
    fn catchup_progress_ignores_pairs_outside_the_configured_slice() {
        let progress = vec![
            row_with_progress(1, 10, true, Some(100.0)),
            row_with_progress(2, 11, false, Some(40.0)),
            row_with_progress(1, 12, false, Some(90.0)),
        ];
        let result = slice_catchup_progress(&progress, Some(&[1]), None);
        assert_eq!(result.min_progress_ratio, Some(0.90));
        assert_eq!(result.pairs_considered, 2);
    }

    #[test]
    fn catchup_progress_ignores_chains_outside_the_configured_slice() {
        let progress = vec![
            row_with_progress(1, 10, true, Some(100.0)),
            row_with_progress(1, 99, false, Some(40.0)),
            row_with_progress(1, 12, false, Some(90.0)),
        ];
        let relevant_chains: BTreeSet<i64> = [10, 12].into_iter().collect();
        let result = slice_catchup_progress(&progress, None, Some(&relevant_chains));
        assert_eq!(result.min_progress_ratio, Some(0.90));
        assert_eq!(result.pairs_considered, 2);
    }

    #[test]
    fn catchup_progress_converts_percent_to_ratio() {
        let progress = vec![row_with_progress(1, 10, false, Some(95.0))];
        let result = slice_catchup_progress(&progress, None, None);
        assert_eq!(result.min_progress_ratio, Some(0.95));
        assert!(is_catchup_progress_sufficient(
            &result,
            &ToggleableThreshold::enabled(0.95)
        ));
        assert!(!is_catchup_progress_sufficient(
            &result,
            &ToggleableThreshold::enabled(0.96)
        ));
    }

    #[test]
    fn catchup_progress_over_no_relevant_pairs_is_unknown() {
        let progress = vec![row_with_progress(1, 10, false, Some(40.0))];
        let result = slice_catchup_progress(&progress, Some(&[99]), None);
        assert_eq!(result.min_progress_ratio, None);
        assert_eq!(result.pairs_considered, 0);
    }

    #[test]
    fn catchup_progress_without_a_reported_percent_is_unknown() {
        let progress = vec![row_with_progress(1, 10, false, None)];
        let result = slice_catchup_progress(&progress, None, None);
        assert_eq!(result.min_progress_ratio, None);
        assert_eq!(result.pairs_missing_progress, 1);
        // the verdict is still well-defined over the same payload
        let verdict = slice_catchup_verdict(&progress, None, None);
        assert!(!verdict.complete);
    }

    #[test]
    fn pair_with_no_checkpoint_row_reports_zero_progress() {
        let progress = vec![row_with_progress(1, 10, false, Some(0.0))];
        let result = slice_catchup_progress(&progress, None, None);
        assert_eq!(result.min_progress_ratio, Some(0.0));
        assert!(!is_catchup_progress_sufficient(
            &result,
            &ToggleableThreshold::enabled(0.01)
        ));
    }

    #[test]
    fn catchup_gate_passes_when_the_threshold_is_disabled() {
        let progress = SliceCatchupProgress {
            min_progress_ratio: Some(0.0),
            pairs_considered: 1,
            pairs_missing_progress: 0,
            slowest: Some((1, 10)),
        };
        assert!(is_catchup_progress_sufficient(
            &progress,
            &ToggleableThreshold::disabled()
        ));
    }

    #[test]
    fn catchup_gate_passes_when_progress_is_unknown() {
        let progress = SliceCatchupProgress {
            min_progress_ratio: None,
            pairs_considered: 0,
            pairs_missing_progress: 0,
            slowest: None,
        };
        assert!(is_catchup_progress_sufficient(
            &progress,
            &ToggleableThreshold::enabled(0.9)
        ));
    }
}
