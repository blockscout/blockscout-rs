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

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::avalanche_data_api::AvalancheDataApiNetwork;

/// RFC 3339, millisecond precision, `Z` suffix -- the one timestamp format
/// every consumer of a stored/served xDai timestamp must agree on.
///
/// Lives here (not in `interchain-indexer-server`) because the
/// `multiple_executions` namespace's `additional_executions[].timestamp`
/// values are assembled in this crate, in
/// `message_buffer::persistence::apply_destination_execution_reconciliation`,
/// while `interchain-indexer-server/src/services/utils.rs::db_datetime_to_string`
/// formats the same kind of value for the rest of the Read API. Two
/// independent copies of `to_rfc3339_opts(...)` would drift silently the first
/// time either one changed; the server function now delegates here instead.
pub fn rfc3339_millis_z(ts: NaiveDateTime) -> String {
    ts.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multiple_executions: Option<MultipleExecutions>,
}

/// Namespace for a canonical message that has been executed on its
/// destination chain more than once. The array holds only the **later**
/// executions -- the canonical one stays exactly where it always was,
/// `InterchainMessage.dst_tx_hash`, and is never duplicated here.
///
/// Shape mirrors [`UnresolvedDestination`]: a universal core
/// (`additional_executions`) plus a flattened protocol discriminant, in the
/// same flat JSON object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultipleExecutions {
    pub additional_executions: Vec<AdditionalExecution>,
    #[serde(flatten)]
    pub protocol: MultipleExecutionsProtocol,
}

/// Internally tagged, matching [`UnresolvedDestinationProtocol`]'s shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "snake_case")]
pub enum MultipleExecutionsProtocol {
    /// Renamed explicitly: `rename_all = "snake_case"` would spell this
    /// variant `x_dai`, and the public JSON of this namespace is `xdai`
    /// (`PublicMetadata::extra_value` below, and the task contract it
    /// implements). Without the rename the same discriminant is spelled two
    /// ways -- `x_dai` in `crosschain_messages.protocol_metadata` and `xdai`
    /// in the API -- which is confusing on its own and is a live trap for any
    /// future change that renders the namespace by reusing `Serialize`
    /// instead of the explicit builder.
    #[serde(rename = "xdai")]
    XDai(XDaiMultipleExecutions),
}

/// No fields of its own today -- xDai has nothing beyond the universal
/// `additional_executions` array. Kept as a struct (not a unit variant) so a
/// future xDai-specific field has somewhere to go without changing the enum
/// shape or the public JSON's `protocol` tag.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XDaiMultipleExecutions {}

/// One later destination execution. Exactly two fields, by design: no block
/// number (nothing for a client to do with it), no chain id (already on
/// `InterchainMessage.dst_chain_id`), no indication of which raw identity kind
/// was observed (an internal `amb_message_anomalies` concern).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdditionalExecution {
    /// 0x-hex, lower case.
    pub transaction_hash: String,
    /// RFC 3339, milliseconds, `Z` -- the same rendering as `send_timestamp`
    /// (and every other served timestamp); see [`rfc3339_millis_z`].
    pub timestamp: String,
}

/// Universal core of the "destination did not resolve" concept. Identical
/// across bridges; everything protocol-specific lives in `protocol`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedDestination {
    /// Verbatim on both directions: whatever the indexer wrote is what the
    /// Read API serves. Deliberately a `String` and not [`UnresolvedReason`]
    /// — nothing reads this field back to branch on it (the upsert merge is
    /// pure SQL and never parses the column), so decoding it into a closed
    /// enum would only buy the ability to *lose* a row's diagnostics when
    /// the wording changes. The vocabulary is constrained where it is
    /// written, by [`UnresolvedReason::as_str`], not where it is read.
    pub reason: String,
    #[serde(flatten)]
    pub protocol: UnresolvedDestinationProtocol,
}

/// Closed, universal set of reasons a destination failed to resolve — the
/// **write-side** vocabulary. Classification produces one of these; storage
/// and serving see only the string it renders to, so this type has no serde
/// impl and changing the wording can never fail to read an existing row.
///
/// Keep the wording protocol-neutral: this is the universal core of the
/// concept, and every bridge that reuses it will be described by these same
/// sentences. Keep it human-readable too — the string is served verbatim to
/// API clients, with no lookup table on their side.
///
/// Not to be confused with the `outcome` label on
/// `AVALANCHE_DESTINATION_RESOLUTION_TOTAL`, which stays snake_case: a
/// metric label is a machine-side dimension, not display text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnresolvedReason {
    /// The protocol registry does not know this identifier.
    UnknownIdentifier,
    /// The identifier is known to the registry, but no chain id is attached
    /// to it.
    NoChainId,
}

impl UnresolvedReason {
    /// The text written into `protocol_metadata`, and from there served in
    /// `extra` unchanged. The only place a reason's wording is defined.
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
        out.insert("reason".to_string(), self.reason.clone().into());
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

impl PublicMetadata for MultipleExecutions {
    const EXTRA_KEY: &'static str = "multiple_executions";

    fn extra_value(&self) -> Value {
        let mut out = Map::new();
        match &self.protocol {
            MultipleExecutionsProtocol::XDai(XDaiMultipleExecutions {}) => {
                out.insert("protocol".to_string(), "xdai".into());
            }
        }
        out.insert(
            "additional_executions".to_string(),
            Value::Array(
                self.additional_executions
                    .iter()
                    .map(|execution| {
                        let mut entry = Map::new();
                        entry.insert(
                            "transaction_hash".to_string(),
                            execution.transaction_hash.clone().into(),
                        );
                        entry.insert("timestamp".to_string(), execution.timestamp.clone().into());
                        Value::Object(entry)
                    })
                    .collect(),
            ),
        );
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
        if let Some(multiple_executions) = &self.multiple_executions {
            out.insert(
                MultipleExecutions::EXTRA_KEY.to_string(),
                multiple_executions.extra_value(),
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
                reason: UnresolvedReason::UnknownIdentifier.as_str().to_owned(),
                protocol: UnresolvedDestinationProtocol::AvalancheIcm(AvalancheIcmDestination {
                    blockchain_id:
                        "0x7fc93d85c6d62c5b2ac0b519c87010ea5294012d1e407030d6acd0021cac10d5"
                            .to_string(),
                    blockchain_id_cb58: "yH8D7ThNJkxmtkuv2jgBa4P1Rn3Qpr4pPr7QYNfcdoS6k6HWp"
                        .to_string(),
                    network: AvalancheDataApiNetwork::Mainnet,
                }),
            }),
            ..Default::default()
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

    /// Whatever sits in the column is what `extra` serves — the reason is
    /// never re-derived from the reading code's own vocabulary. Covers the
    /// snake_case spelling this field shipped with, a reason a newer indexer
    /// might write, and today's wording.
    #[rstest]
    #[case("unknown_identifier")]
    #[case("some_future_reason_this_build_never_heard_of")]
    #[case("Unable to resolve the destination chain")]
    fn stored_reason_is_served_verbatim(#[case] stored: &str) {
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
        assert_eq!(unresolved.reason, stored);
        assert_eq!(
            unresolved.extra_value()["reason"],
            serde_json::json!(stored),
            "the Read API must not rewrite a stored reason"
        );
    }

    /// The write path is where the vocabulary is constrained: every variant
    /// renders to the sentence it is served as.
    #[test]
    fn every_reason_variant_renders_its_own_sentence() {
        assert_eq!(
            UnresolvedReason::UnknownIdentifier.as_str(),
            "Unable to resolve the destination chain"
        );
        assert_eq!(
            UnresolvedReason::NoChainId.as_str(),
            "The destination chain has no EVM chain ID"
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

    // --- `multiple_executions` namespace ---

    fn multiple_executions_sample() -> ProtocolMetadata {
        ProtocolMetadata {
            multiple_executions: Some(MultipleExecutions {
                additional_executions: vec![AdditionalExecution {
                    transaction_hash:
                        "0x2e50d68b6d0bc152a0503e6782adcfead09fe9de98eabd852d4055d15efe2512"
                            .to_string(),
                    timestamp: "2025-07-17T20:52:50.000Z".to_string(),
                }],
                protocol: MultipleExecutionsProtocol::XDai(XDaiMultipleExecutions {}),
            }),
            ..Default::default()
        }
    }

    /// Written first, per the task's instruction: if `#[serde(flatten)]` over
    /// an internally tagged enum with an empty payload struct does not
    /// round-trip, the fallback is a unit variant `XDai` instead of
    /// `XDai(XDaiMultipleExecutions)` -- the public JSON is identical either
    /// way, only the Rust shape changes.
    #[test]
    fn multiple_executions_round_trips() {
        let metadata = multiple_executions_sample();
        let value = metadata.to_json_value().expect("non-empty metadata");
        let round_tripped = ProtocolMetadata::from_json_value(Some(value)).expect("decodes");
        assert_eq!(round_tripped, metadata);
    }

    /// The exact documented JSON shape: two fields per entry, `protocol` at
    /// the top of the namespace object, no block number, no chain id, no
    /// alias-kind indicator.
    #[test]
    fn render_extra_renders_the_documented_multiple_executions_json() {
        let extra = Value::Object(multiple_executions_sample().render_extra());
        let expected = serde_json::json!({
            "multiple_executions": {
                "protocol": "xdai",
                "additional_executions": [
                    {
                        "transaction_hash": "0x2e50d68b6d0bc152a0503e6782adcfead09fe9de98eabd852d4055d15efe2512",
                        "timestamp": "2025-07-17T20:52:50.000Z"
                    }
                ]
            }
        });
        assert_eq!(extra, expected);
    }

    /// The stored discriminant and the rendered one must be the same string.
    /// They are produced by two independent code paths -- `Serialize` for the
    /// JSONB column, an explicit `Map` builder for the API -- so nothing but a
    /// test keeps them from drifting apart.
    #[test]
    fn stored_and_rendered_protocol_discriminants_agree() {
        let metadata = multiple_executions_sample();
        let stored = metadata.to_json_value().expect("non-empty metadata");
        let stored_tag = stored
            .pointer("/multiple_executions/protocol")
            .expect("stored namespace carries the discriminant");
        let rendered = Value::Object(metadata.render_extra());
        let rendered_tag = rendered
            .pointer("/multiple_executions/protocol")
            .expect("rendered namespace carries the discriminant");
        assert_eq!(stored_tag, "xdai");
        assert_eq!(stored_tag, rendered_tag);
    }

    #[test]
    fn multiple_executions_absent_from_default_metadata() {
        assert_eq!(ProtocolMetadata::default().to_json_value(), None);
        assert!(ProtocolMetadata::default().render_extra().is_empty());
    }

    #[test]
    fn rfc3339_millis_z_matches_the_documented_format() {
        let ts = chrono::DateTime::from_timestamp(1_752_785_570, 0)
            .unwrap()
            .naive_utc();
        assert_eq!(rfc3339_millis_z(ts), "2025-07-17T20:52:50.000Z");
    }
}
