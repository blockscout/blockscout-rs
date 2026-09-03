// SPDX-License-Identifier: LicenseRef-Blockscout

//! Log initialization with built-in noise suppression for third-party targets.
//!
//! Wraps [`blockscout_service_launcher::tracing::init_logs_with_filter`] so the
//! per-RPC-call chatter of the Alloy HTTP transport is silenced by default,
//! without requiring every deployment to carry a `RUST_LOG` override.

use blockscout_service_launcher::tracing::{
    JaegerSettings, TracingSettings, init_logs_with_filter,
};
use tracing::{Level, Metadata};

/// Targets whose INFO-level output is pure per-request noise for this service.
///
/// `alloy_transport_http` wraps every JSON-RPC call in an
/// `#[instrument(name = "request")]` span, and `#[instrument]` defaults to the
/// INFO level, so the launcher's fmt layer (`FmtSpan::NEW | FmtSpan::CLOSE`)
/// prints two lines per RPC call. At the indexer's request rate that buries
/// every other log line.
///
/// Matching follows `RUST_LOG` semantics: an entry matches the target itself
/// and any `::`-separated child of it.
const NOISY_TARGETS: &[&str] = &["alloy_transport_http"];

/// Highest level a [`NOISY_TARGETS`] entry is still allowed to log at.
/// WARN and ERROR always pass through — they carry real RPC failures.
const NOISY_TARGET_MAX_LEVEL: Level = Level::WARN;

/// Initializes logs for the service, suppressing [`NOISY_TARGETS`].
///
/// Signature-compatible with
/// [`blockscout_service_launcher::tracing::init_logs`]; the suppression is a
/// second layer filter on top of the launcher's `EnvFilter`, so `RUST_LOG`
/// still governs everything else.
pub fn init_logs(
    service_name: &str,
    tracing_settings: &TracingSettings,
    jaeger_settings: &JaegerSettings,
) -> Result<(), anyhow::Error> {
    let suppressed = suppressed_targets(std::env::var("RUST_LOG").unwrap_or_default().as_str());

    init_logs_with_filter(
        service_name,
        tracing_settings,
        jaeger_settings,
        tracing_subscriber::filter::filter_fn(move |meta: &Metadata<'_>| {
            !is_suppressed(&suppressed, meta.target(), *meta.level())
        }),
    )
}

/// Drops from [`NOISY_TARGETS`] anything named in `RUST_LOG`.
///
/// An explicit directive wins over the built-in default, so
/// `RUST_LOG=info,alloy_transport_http=debug` still works when someone is
/// debugging RPC traffic. The substring check is deliberately coarse: it only
/// decides whether to step out of `RUST_LOG`'s way.
fn suppressed_targets(rust_log: &str) -> Vec<&'static str> {
    NOISY_TARGETS
        .iter()
        .copied()
        .filter(|target| !rust_log.contains(target))
        .collect()
}

fn is_suppressed(suppressed: &[&str], target: &str, level: Level) -> bool {
    level > NOISY_TARGET_MAX_LEVEL && suppressed.iter().any(|s| matches_target(s, target))
}

/// `RUST_LOG`-style prefix match: `alloy_transport_http` matches itself and
/// `alloy_transport_http::reqwest_transport`, but not `alloy_transport_httpx`.
fn matches_target(suppressed: &str, target: &str) -> bool {
    target
        .strip_prefix(suppressed)
        .is_some_and(|rest| rest.is_empty() || rest.starts_with("::"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUPPRESSED: &[&str] = &["alloy_transport_http"];

    #[test]
    fn test_is_suppressed_noisy_target_info_and_below_is_suppressed() {
        for level in [Level::INFO, Level::DEBUG, Level::TRACE] {
            assert!(
                is_suppressed(SUPPRESSED, "alloy_transport_http::reqwest_transport", level),
                "{level} should be suppressed"
            );
        }
    }

    #[test]
    fn test_is_suppressed_noisy_target_warn_and_above_passes() {
        for level in [Level::WARN, Level::ERROR] {
            assert!(
                !is_suppressed(SUPPRESSED, "alloy_transport_http::reqwest_transport", level),
                "{level} must not be suppressed"
            );
        }
    }

    #[test]
    fn test_is_suppressed_other_targets_are_untouched() {
        assert!(!is_suppressed(
            SUPPRESSED,
            "interchain_indexer_logic::indexer",
            Level::INFO
        ));
        // Prefix match must respect module boundaries.
        assert!(!is_suppressed(
            SUPPRESSED,
            "alloy_transport_httpx",
            Level::INFO
        ));
    }

    #[test]
    fn test_is_suppressed_empty_list_suppresses_nothing() {
        assert!(!is_suppressed(
            &[],
            "alloy_transport_http::reqwest_transport",
            Level::INFO
        ));
    }

    #[test]
    fn test_suppressed_targets_rust_log_directive_wins() {
        assert_eq!(suppressed_targets(""), NOISY_TARGETS.to_vec());
        assert_eq!(suppressed_targets("info"), NOISY_TARGETS.to_vec());
        assert!(suppressed_targets("info,alloy_transport_http=debug").is_empty());
    }
}
