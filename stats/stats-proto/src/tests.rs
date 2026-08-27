// SPDX-License-Identifier: LicenseRef-Blockscout

use prost::Message;

use crate::blockscout::stats::v1::{self as proto};

const PRECISE_POINT_1: &str = r#"
{
    "date": "2024-03-14",
    "date_to": "2024-03-14",
    "value": "188542399",
    "is_approximate": false
}
"#;

const PRECISE_POINT_2: &str = r#"
{
    "date": "2024-03-14",
    "date_to": "2024-03-14",
    "value": "188542399"
}
"#;

const IMPRECISE_POINT: &str = r#"
{
    "date": "2024-03-14",
    "date_to": "2024-03-14",
    "value": "188542399",
    "is_approximate": true
}
"#;

#[test]
fn is_approximate_serialization() {
    // deserialize
    let point: proto::Point = serde_json::from_str(PRECISE_POINT_1).unwrap();
    assert!(!point.is_approximate);
    let point: proto::Point = serde_json::from_str(PRECISE_POINT_2).unwrap();
    assert!(!point.is_approximate);
    let point: proto::Point = serde_json::from_str(IMPRECISE_POINT).unwrap();
    assert!(point.is_approximate);

    // serialize
    let point = proto::Point {
        date: "2024-03-14".to_owned(),
        date_to: "2024-03-14".to_owned(),
        value: "188542399".to_owned(),
        is_approximate: false,
    };
    let serialized_point = serde_json::to_string(&point).unwrap();
    assert_eq!(
        serialized_point.replace([' ', '\n'], ""),
        PRECISE_POINT_2.replace([' ', '\n'], "")
    );
    let point = proto::Point {
        date: "2024-03-14".to_owned(),
        date_to: "2024-03-14".to_owned(),
        value: "188542399".to_owned(),
        is_approximate: true,
    };
    let serialized_point = serde_json::to_string(&point).unwrap();
    assert_eq!(
        serialized_point.replace([' ', '\n'], ""),
        IMPRECISE_POINT.replace([' ', '\n'], "")
    );
}

fn update_status(interchain_history_catching_up: Option<bool>) -> proto::UpdateStatus {
    proto::UpdateStatus {
        all_status: proto::ChartSubsetUpdateStatus::Pending.into(),
        independent_status: proto::ChartSubsetUpdateStatus::Pending.into(),
        blocks_dependent_status: proto::ChartSubsetUpdateStatus::Pending.into(),
        internal_transactions_dependent_status: proto::ChartSubsetUpdateStatus::Pending.into(),
        user_ops_dependent_status: proto::ChartSubsetUpdateStatus::Pending.into(),
        zetachain_cctx_dependent_status: proto::ChartSubsetUpdateStatus::Pending.into(),
        interchain_history_catching_up,
    }
}

/// Pins the whole "unknown ≠ false" decision: an absent verdict must serialize
/// as a missing key, never as `null`.
#[test]
fn update_status_omits_absent_interchain_catching_up() {
    let json = serde_json::to_string(&update_status(None)).unwrap();
    assert!(
        !json.contains("interchain_history_catching_up"),
        "the key must not appear at all when the field is None: {json}"
    );
}

/// Proves a *known* `false` is still transmitted, so absence really does mean
/// unknown rather than "we checked and it's false".
#[test]
fn update_status_serializes_known_interchain_catching_up() {
    let json = serde_json::to_string(&update_status(Some(true))).unwrap();
    assert!(
        json.contains(r#""interchain_history_catching_up":true"#),
        "{json}"
    );

    let json = serde_json::to_string(&update_status(Some(false))).unwrap();
    assert!(
        json.contains(r#""interchain_history_catching_up":false"#),
        "{json}"
    );
}

/// An older linked hop that predates this field sends a six-field body; it
/// must parse fine, with the field resolving to `None`, not a deserialization
/// error.
#[test]
fn update_status_deserializes_without_interchain_catching_up() {
    let json = r#"{
        "all_status": "PENDING",
        "independent_status": "PENDING",
        "blocks_dependent_status": "PENDING",
        "internal_transactions_dependent_status": "PENDING",
        "user_ops_dependent_status": "PENDING",
        "zetachain_cctx_dependent_status": "PENDING"
    }"#;
    let status: proto::UpdateStatus = serde_json::from_str(json).unwrap();
    assert_eq!(status.interchain_history_catching_up, None);
}
