use prost::Message;

use crate::blockscout::stats::v1::{self as proto};

const PRECISE_POINT_1: &str = r#"
{
    "date": "2024-03-14",
    "value": "188542399",
    "is_approximate": false
}
"#;

const PRECISE_POINT_2: &str = r#"
{
    "date": "2024-03-14",
    "value": "188542399"
}
"#;

const IMPRECISE_POINT: &str = r#"
{
    "date": "2024-03-14",
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
        value: "188542399".to_owned(),
        is_approximate: true,
    };
    let serialized_point = serde_json::to_string(&point).unwrap();
    assert_eq!(
        serialized_point.replace([' ', '\n'], ""),
        IMPRECISE_POINT.replace([' ', '\n'], "")
    );
}
