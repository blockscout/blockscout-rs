// SPDX-License-Identifier: LicenseRef-Blockscout

//! Sparse, protocol-specific metadata for a canonical `crosschain_messages`
//! row, stored in the `protocol_metadata` JSONB column.
//!
//! Namespaces are additive: a new concept is a new field on
//! [`ProtocolMetadata`], not a rewrite of existing ones. Whatever a given row
//! actually has is what gets serialized — see [`ProtocolMetadata::to_json_value`].

use std::{collections::BTreeMap, str::FromStr};

use serde::{Deserialize, Serialize};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnresolvedReason {
    /// The protocol registry does not know this identifier.
    UnknownIdentifier,
    /// The identifier is known to the registry, but no chain id is attached
    /// to it.
    NoChainId,
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
    /// Prefix for keys in `extra`. Matches the namespace's name in JSON.
    const EXTRA_PREFIX: &'static str;

    fn render_extra(&self, out: &mut BTreeMap<String, String>);
}

impl PublicMetadata for UnresolvedDestination {
    const EXTRA_PREFIX: &'static str = "unresolved_destination";

    fn render_extra(&self, out: &mut BTreeMap<String, String>) {
        let prefix = Self::EXTRA_PREFIX;
        out.insert(
            format!("{prefix}.reason"),
            match self.reason {
                UnresolvedReason::UnknownIdentifier => "unknown_identifier".to_string(),
                UnresolvedReason::NoChainId => "no_chain_id".to_string(),
            },
        );
        match &self.protocol {
            UnresolvedDestinationProtocol::AvalancheIcm(avalanche) => {
                out.insert(format!("{prefix}.protocol"), "avalanche_icm".to_string());
                out.insert(
                    format!("{prefix}.blockchain_id"),
                    avalanche.blockchain_id.clone(),
                );
                out.insert(
                    format!("{prefix}.blockchain_id_cb58"),
                    avalanche.blockchain_id_cb58.clone(),
                );
                out.insert(
                    format!("{prefix}.network"),
                    avalanche.network.as_ref().to_string(),
                );
            }
        }
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

    /// Explicit public render, only via [`PublicMetadata`].
    pub fn render_extra(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        if let Some(unresolved_destination) = &self.unresolved_destination {
            unresolved_destination.render_extra(&mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
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
                "reason": "unknown_identifier",
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
    fn render_extra_has_exactly_the_five_documented_keys() {
        let extra = sample().render_extra();
        let keys: Vec<&str> = extra.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            vec![
                "unresolved_destination.blockchain_id",
                "unresolved_destination.blockchain_id_cb58",
                "unresolved_destination.network",
                "unresolved_destination.protocol",
                "unresolved_destination.reason",
            ]
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
                "reason": "unknown_identifier",
                "protocol": "some_other_protocol",
                "foo": "bar",
            }
        });
        assert_eq!(ProtocolMetadata::from_json_value(Some(value)), None);
    }
}
