// SPDX-License-Identifier: LicenseRef-Blockscout

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{indexer::failure_ledger::settings::FailureRetrySettings, log_stream::HeadBlockTag};

#[serde_as]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct OpInteropIndexerSettings {
    #[serde(default = "default_pull_interval")]
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub pull_interval_ms: Duration,
    #[serde(default = "default_batch_size")]
    pub batch_size: u64,
    #[serde(default)]
    pub confirmation_block_tag: ConfirmationBlockTag,
    #[serde(default)]
    pub failure_retry: FailureRetrySettings,
}

impl Default for OpInteropIndexerSettings {
    fn default() -> Self {
        Self {
            pull_interval_ms: default_pull_interval(),
            batch_size: default_batch_size(),
            confirmation_block_tag: ConfirmationBlockTag::default(),
            failure_retry: FailureRetrySettings::default(),
        }
    }
}

/// The block tag up to which this indexer reads. `latest` is deliberately
/// not representable: the indexer has no rollback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationBlockTag {
    Safe,
    #[default]
    Finalized,
}

impl ConfirmationBlockTag {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Finalized => "finalized",
        }
    }
}

impl From<ConfirmationBlockTag> for HeadBlockTag {
    fn from(tag: ConfirmationBlockTag) -> Self {
        match tag {
            ConfirmationBlockTag::Safe => Self::Safe,
            ConfirmationBlockTag::Finalized => Self::Finalized,
        }
    }
}

/// One head request plus one log request per interval keeps a chain near one
/// call per second in steady state (the block time is 2 s).
fn default_pull_interval() -> Duration {
    Duration::from_millis(2000)
}

/// Clear of the 1000-block `eth_getLogs` cap seen on OP Stack RPCs.
fn default_batch_size() -> u64 {
    500
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_op_interop_settings_defaults() {
        let settings: OpInteropIndexerSettings =
            serde_json::from_value(serde_json::json!({})).expect("empty settings deserialize");

        assert_eq!(settings, OpInteropIndexerSettings::default());
        assert_eq!(settings.pull_interval_ms, Duration::from_millis(2000));
        assert_eq!(settings.batch_size, 500);
        assert_eq!(
            settings.confirmation_block_tag,
            ConfirmationBlockTag::Finalized
        );
        assert_eq!(settings.failure_retry, FailureRetrySettings::default());
        assert_eq!(
            HeadBlockTag::from(settings.confirmation_block_tag),
            HeadBlockTag::Finalized
        );
    }

    #[test]
    fn test_confirmation_block_tag_accepts_safe_and_finalized() {
        for (raw, expected, head) in [
            ("safe", ConfirmationBlockTag::Safe, HeadBlockTag::Safe),
            (
                "finalized",
                ConfirmationBlockTag::Finalized,
                HeadBlockTag::Finalized,
            ),
        ] {
            let settings: OpInteropIndexerSettings =
                serde_json::from_value(serde_json::json!({"confirmation_block_tag": raw}))
                    .expect("tag deserializes");
            assert_eq!(settings.confirmation_block_tag, expected);
            assert_eq!(settings.confirmation_block_tag.as_str(), raw);
            assert_eq!(HeadBlockTag::from(settings.confirmation_block_tag), head);
        }
    }

    #[test]
    fn test_confirmation_block_tag_rejects_latest() {
        for raw in ["latest", "Finalized", "finalised", ""] {
            let result: Result<OpInteropIndexerSettings, _> =
                serde_json::from_value(serde_json::json!({"confirmation_block_tag": raw}));
            assert!(result.is_err(), "{raw:?} must not be accepted as a tag");
        }
    }
}
