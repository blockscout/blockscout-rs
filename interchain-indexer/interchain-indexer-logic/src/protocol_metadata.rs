// SPDX-License-Identifier: LicenseRef-Blockscout

//! Sparse, protocol-specific metadata for a canonical `crosschain_messages`
//! row, stored in the `protocol_metadata` JSONB column.
//!
//! Namespaces are additive: a new concept is a new field on
//! [`ProtocolMetadata`], not a rewrite of existing ones. Whatever a given row
//! actually has is what gets serialized — see [`ProtocolMetadata::to_json_value`].
//!
//! The same namespacing reaches the Read API: [`ProtocolMetadata::render_extra`]
//! puts each public namespace under its own key in `InterchainMessage.extra`,
//! as a nested JSON object rather than a set of dotted flat keys.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::avalanche_data_api::AvalancheDataApiNetwork;

/// `AvalancheDataApiNetwork` derives `Serialize`/`Deserialize` without
/// `rename_all` because it also backs `AvalancheDataApiClientSettings`, a
/// config field — changing its JSON casing there is a config-surface change
/// out of scope for this task. Render/parse it here via the `AsRefStr` /
/// `EnumString` impls it already has for URL building, which are lowercase
/// and case-insensitive, so `extra`/`protocol_metadata` get "mainnet" rather
/// than the derive's default "Mainnet".
mod network_as_lowercase_str {
    use super::{AvalancheDataApiNetwork, FromStr};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        network: &AvalancheDataApiNetwork,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(network.as_ref())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<AvalancheDataApiNetwork, D::Error> {
        let s = String::deserialize(deserializer)?;
        AvalancheDataApiNetwork::from_str(&s).map_err(serde::de::Error::custom)
    }
}

/// Sparse protocol metadata for a canonical message.
///
/// Top-level keys are concept namespaces. Only what is actually present is
/// serialized: an empty container yields `{}`, which the caller must turn
/// into SQL NULL (see [`ProtocolMetadata::to_json_value`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unresolved_destination: Option<UnresolvedDestination>,
}

/// Universal core of the "destination did not resolve" concept. Identical
/// across bridges; everything protocol-specific lives in `protocol`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedDestination {
    pub reason: UnresolvedReason,
    #[serde(flatten)]
    pub protocol: UnresolvedDestinationProtocol,
}

/// Closed, universal set of reasons a destination failed to resolve.
///
/// The JSON spelling of each variant is a sentence a human can read without
/// a lookup table, because it is what `InterchainMessage.extra` hands to a
/// client verbatim — the same text is what gets stored, so
/// [`ProtocolMetadata`] rows read back the way they are served. Keep the
/// wording protocol-neutral: this enum is the universal core of the concept,
/// and every bridge that ever reuses it will be described by these same
/// sentences.
///
/// Each variant also carries an `alias` for the snake_case spelling this
/// enum shipped with, so rows written before the wording change still decode
/// instead of silently losing their diagnostics. Any later re-wording must
/// add its predecessor the same way.
///
/// Not to be confused with the `outcome` label on
/// `AVALANCHE_DESTINATION_RESOLUTION_TOTAL`, which stays snake_case: a
/// metric label is a machine-side dimension, not display text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnresolvedReason {
    /// The protocol registry does not know this identifier.
    #[serde(
        rename = "Unable to resolve the destination chain",
        alias = "unknown_identifier"
    )]
    UnknownIdentifier,
    /// The identifier is known to the registry, but no chain id is attached
    /// to it.
    #[serde(
        rename = "The destination chain has no EVM chain ID",
        alias = "no_chain_id"
    )]
    NoChainId,
}

impl UnresolvedReason {
    /// The one spelling of this reason: stored in `protocol_metadata` and
    /// served in `extra`. Kept in sync with the `serde` attributes above by
    /// `serde_spelling_matches_as_str`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnknownIdentifier => "Unable to resolve the destination chain",
            Self::NoChainId => "The destination chain has no EVM chain ID",
        }
    }
}

/// Internally tagged: the `protocol` discriminant sits alongside the
/// variant's own fields, so the JSON stays flat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "snake_case")]
pub enum UnresolvedDestinationProtocol {
    AvalancheIcm(AvalancheIcmDestination),
}

/// Fields named the way the protocol itself names them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvalancheIcmDestination {
    /// `destinationBlockchainID` from the event, 0x-hex.
    pub blockchain_id: String,
    /// The same value in CB58 — the form Avalanche tooling shows it in, and
    /// the form the Data API looks it up by.
    pub blockchain_id_cb58: String,
    /// The Avalanche Data API network the lookup was performed against.
    #[serde(with = "network_as_lowercase_str")]
    pub network: AvalancheDataApiNetwork,
}

/// Namespace that is allowed to be handed to a client.
///
/// Default-deny: a namespace without this impl cannot be rendered into
/// `InterchainMessage.extra` without deliberately writing an impl. Internal
/// indexer metadata simply does not implement the trait — and so cannot leak.
pub trait PublicMetadata {
    /// The namespace's key in `extra`. Matches its name in JSON.
    const EXTRA_KEY: &'static str;

    /// The namespace's public JSON object, exactly as a client sees it under
    /// [`Self::EXTRA_KEY`]. Built field by field on purpose: the stored
    /// payload may grow fields that must not be served, so `Serialize` on the
    /// storage type is deliberately not reused here.
    fn extra_value(&self) -> Value;
}

impl PublicMetadata for UnresolvedDestination {
    const EXTRA_KEY: &'static str = "unresolved_destination";

    fn extra_value(&self) -> Value {
        let mut out = Map::new();
        out.insert("reason".to_string(), self.reason.as_str().into());
        match &self.protocol {
            UnresolvedDestinationProtocol::AvalancheIcm(avalanche) => {
                out.insert("protocol".to_string(), "avalanche_icm".into());
                out.insert(
                    "blockchain_id".to_string(),
                    avalanche.blockchain_id.clone().into(),
                );
                out.insert(
                    "blockchain_id_cb58".to_string(),
                    avalanche.blockchain_id_cb58.clone().into(),
                );
                out.insert(
                    "network".to_string(),
                    avalanche.network.as_ref().to_string().into(),
                );
            }
        }
        Value::Object(out)
    }
}

impl ProtocolMetadata {
    /// `None` when the container is empty — the column must stay SQL NULL.
    /// An empty `{}` object must never reach the database.
    pub fn to_json_value(&self) -> Option<serde_json::Value> {
        if *self == Self::default() {
            return None;
        }
        serde_json::to_value(self).ok()
    }

    /// Tolerant read: a malformed or unfamiliar value never fails the
    /// request. On error, `tracing::warn!` and `None` — the error text is
    /// never surfaced to callers (`error-handling.md`: no internal details in
    /// API responses).
    pub fn from_json_value(value: Option<serde_json::Value>) -> Option<Self> {
        let value = value?;
        match serde_json::from_value(value) {
            Ok(metadata) => Some(metadata),
            Err(err) => {
                tracing::warn!(err = ?err, "failed to deserialize protocol_metadata");
                None
            }
        }
    }

    /// Explicit public render, only via [`PublicMetadata`]. One entry per
    /// present namespace, each holding that namespace's own JSON object.
    pub fn render_extra(&self) -> Map<String, Value> {
        let mut out = Map::new();
        if let Some(unresolved_destination) = &self.unresolved_destination {
            out.insert(
                UnresolvedDestination::EXTRA_KEY.to_string(),
                unresolved_destination.extra_value(),
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn sample() -> ProtocolMetadata {
        ProtocolMetadata {
            unresolved_destination: Some(UnresolvedDestination {
                reason: UnresolvedReason::UnknownIdentifier,
                protocol: UnresolvedDestinationProtocol::AvalancheIcm(AvalancheIcmDestination {
                    blockchain_id:
                        "0x7fc93d85c6d62c5b2ac0b519c87010ea5294012d1e407030d6acd0021cac10d5"
                            .to_string(),
                    blockchain_id_cb58: "yH8D7ThNJkxmtkuv2jgBa4P1Rn3Qpr4pPr7QYNfcdoS6k6HWp"
                        .to_string(),
                    network: AvalancheDataApiNetwork::Mainnet,
                }),
            }),
        }
    }

    #[test]
    fn round_trip_serializes_flat_with_reason_and_protocol_fields_at_same_level() {
        let metadata = sample();
        let value = metadata.to_json_value().expect("non-empty metadata");
        let expected = serde_json::json!({
            "unresolved_destination": {
                "reason": "Unable to resolve the destination chain",
                "protocol": "avalanche_icm",
                "blockchain_id": "0x7fc93d85c6d62c5b2ac0b519c87010ea5294012d1e407030d6acd0021cac10d5",
                "blockchain_id_cb58": "yH8D7ThNJkxmtkuv2jgBa4P1Rn3Qpr4pPr7QYNfcdoS6k6HWp",
                "network": "mainnet",
            }
        });
        assert_eq!(value, expected);

        let round_tripped = ProtocolMetadata::from_json_value(Some(value)).expect("decodes");
        assert_eq!(round_tripped, metadata);
    }

    #[test]
    fn empty_container_has_no_json_value() {
        assert_eq!(ProtocolMetadata::default().to_json_value(), None);
    }

    #[test]
    fn render_extra_nests_the_namespace_as_one_json_object() {
        let extra = Value::Object(sample().render_extra());
        let expected = serde_json::json!({
            "unresolved_destination": {
                "reason": "Unable to resolve the destination chain",
                "protocol": "avalanche_icm",
                "blockchain_id":
                    "0x7fc93d85c6d62c5b2ac0b519c87010ea5294012d1e407030d6acd0021cac10d5",
                "blockchain_id_cb58": "yH8D7ThNJkxmtkuv2jgBa4P1Rn3Qpr4pPr7QYNfcdoS6k6HWp",
                "network": "mainnet",
            }
        });
        assert_eq!(extra, expected);
    }

    /// The stored spelling and the served spelling are the same string by
    /// design, so the `serde` attributes and `as_str` must never drift.
    #[test]
    fn serde_spelling_matches_as_str() {
        for reason in [
            UnresolvedReason::UnknownIdentifier,
            UnresolvedReason::NoChainId,
        ] {
            assert_eq!(
                serde_json::to_value(reason).expect("reason serializes"),
                serde_json::json!(reason.as_str()),
                "serde and as_str disagree for {reason:?}"
            );
            assert_eq!(
                serde_json::from_value::<UnresolvedReason>(serde_json::json!(reason.as_str()))
                    .expect("reason round-trips"),
                reason
            );
        }
    }

    /// Rows written before the reasons were reworded must keep decoding —
    /// otherwise a tolerant read turns them into missing diagnostics.
    #[rstest]
    #[case("unknown_identifier", UnresolvedReason::UnknownIdentifier)]
    #[case("no_chain_id", UnresolvedReason::NoChainId)]
    fn legacy_snake_case_reason_still_decodes(
        #[case] stored: &str,
        #[case] expected: UnresolvedReason,
    ) {
        let value = serde_json::json!({
            "unresolved_destination": {
                "reason": stored,
                "protocol": "avalanche_icm",
                "blockchain_id": "0xaa",
                "blockchain_id_cb58": "cb58-placeholder",
                "network": "mainnet",
            }
        });
        let metadata = ProtocolMetadata::from_json_value(Some(value)).expect("decodes");
        let unresolved = metadata
            .unresolved_destination
            .expect("namespace is present");
        assert_eq!(unresolved.reason, expected);
        assert_eq!(
            unresolved.extra_value()["reason"],
            serde_json::json!(expected.as_str()),
            "a legacy row must be served with the current wording"
        );
    }

    #[test]
    fn render_extra_of_empty_container_is_empty() {
        assert!(ProtocolMetadata::default().render_extra().is_empty());
    }

    #[test]
    fn unknown_top_level_namespace_is_ignored_on_read_and_absent_from_extra() {
        let value = serde_json::json!({
            "some_future_namespace": {"foo": "bar"}
        });
        let metadata = ProtocolMetadata::from_json_value(Some(value)).expect("decodes");
        assert_eq!(metadata, ProtocolMetadata::default());
        assert!(metadata.render_extra().is_empty());
    }

    #[test]
    fn unknown_protocol_tag_fails_to_decode_without_panicking() {
        let value = serde_json::json!({
            "unresolved_destination": {
                "reason": "Unable to resolve the destination chain",
                "protocol": "some_other_protocol",
                "foo": "bar",
            }
        });
        assert_eq!(ProtocolMetadata::from_json_value(Some(value)), None);
    }
}
