// SPDX-License-Identifier: LicenseRef-Blockscout

use chrono::NaiveDateTime;
use sea_orm::JsonValue;
use tonic::Status;

pub fn db_datetime_to_string(ts: NaiveDateTime) -> String {
    ts.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn map_db_error(err: anyhow::Error) -> tonic::Status {
    tracing::error!("database error: {:?}", err);
    tonic::Status::internal("internal server error")
}

pub fn sort_json_value(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut sorted = serde_json::Map::new();
            for (key, value) in entries {
                sorted.insert(key, sort_json_value(value));
            }
            JsonValue::Object(sorted)
        }
        JsonValue::Array(values) => {
            JsonValue::Array(values.into_iter().map(sort_json_value).collect())
        }
        other => other,
    }
}

pub fn parse_chain_ids_csv(input: Option<&str>) -> Result<Vec<i64>, Status> {
    let Some(input) = input.map(str::trim) else {
        return Ok(Vec::new());
    };
    if input.is_empty() {
        return Ok(Vec::new());
    }

    input
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<i64>().map_err(|_| {
                Status::invalid_argument(format!(
                    "invalid chain_ids value `{part}`: expected comma-separated int64 ids"
                ))
            })
        })
        .collect()
}

pub fn parse_bridge_ids_csv(input: Option<&str>) -> Result<Vec<i32>, Status> {
    let Some(input) = input.map(str::trim) else {
        return Ok(Vec::new());
    };
    if input.is_empty() {
        return Ok(Vec::new());
    }

    input
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<i32>().map_err(|_| {
                Status::invalid_argument(format!(
                    "invalid bridge_ids value `{part}`: expected comma-separated int32 ids"
                ))
            })
        })
        .collect()
}

pub fn non_empty<T>(v: Vec<T>) -> Option<Vec<T>> {
    if v.is_empty() { None } else { Some(v) }
}

pub fn reject_unsupported(param: &str, value: Option<&str>) -> Result<(), Status> {
    match value.map(str::trim) {
        Some(v) if !v.is_empty() => Err(Status::invalid_argument(format!(
            "{param} is not supported by this endpoint yet"
        ))),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{non_empty, parse_bridge_ids_csv, parse_chain_ids_csv, reject_unsupported};

    #[test]
    fn parse_chain_ids_csv_accepts_missing_and_empty() {
        assert_eq!(parse_chain_ids_csv(None).unwrap(), Vec::<i64>::new());
        assert_eq!(parse_chain_ids_csv(Some("")).unwrap(), Vec::<i64>::new());
        assert_eq!(parse_chain_ids_csv(Some("   ")).unwrap(), Vec::<i64>::new());
    }

    #[test]
    fn parse_chain_ids_csv_parses_comma_separated_ids() {
        assert_eq!(
            parse_chain_ids_csv(Some("123,456, 789")).unwrap(),
            vec![123, 456, 789]
        );
    }

    #[test]
    fn parse_chain_ids_csv_rejects_invalid_values() {
        let err = parse_chain_ids_csv(Some("123,abc")).expect_err("must reject invalid id");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("invalid chain_ids value `abc`"));
    }

    #[test]
    fn parse_bridge_ids_csv_accepts_missing_and_empty() {
        assert_eq!(parse_bridge_ids_csv(None).unwrap(), Vec::<i32>::new());
        assert_eq!(parse_bridge_ids_csv(Some("")).unwrap(), Vec::<i32>::new());
        assert_eq!(
            parse_bridge_ids_csv(Some("   ")).unwrap(),
            Vec::<i32>::new()
        );
    }

    #[test]
    fn parse_bridge_ids_csv_parses_comma_separated_ids() {
        assert_eq!(parse_bridge_ids_csv(Some("1,2, 3")).unwrap(), vec![1, 2, 3]);
    }

    #[test]
    fn parse_bridge_ids_csv_rejects_invalid_values() {
        let err = parse_bridge_ids_csv(Some("1,abc")).expect_err("must reject invalid id");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("invalid bridge_ids value `abc`"));
    }

    #[test]
    fn parse_bridge_ids_csv_rejects_i32_overflow() {
        let err = parse_bridge_ids_csv(Some("3000000000")).expect_err("must reject i32 overflow");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(
            err.message()
                .contains("invalid bridge_ids value `3000000000`")
        );
    }

    #[test]
    fn non_empty_maps_empty_to_none() {
        assert_eq!(non_empty(Vec::<i32>::new()), None);
        assert_eq!(non_empty(vec![1i32]), Some(vec![1]));
    }

    #[test]
    fn reject_unsupported_rejects_non_blank() {
        let err = reject_unsupported("bridge_ids", Some("1")).expect_err("must reject");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(
            err.message()
                .contains("bridge_ids is not supported by this endpoint yet")
        );
        let err = reject_unsupported("bridge_ids", Some("  1  ")).expect_err("must reject");
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
    }

    #[test]
    fn reject_unsupported_allows_blank_and_none() {
        reject_unsupported("bridge_ids", None).unwrap();
        reject_unsupported("bridge_ids", Some("")).unwrap();
        reject_unsupported("bridge_ids", Some("   ")).unwrap();
    }
}
