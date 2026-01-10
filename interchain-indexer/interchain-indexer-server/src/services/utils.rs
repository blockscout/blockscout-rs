use chrono::NaiveDateTime;
use sea_orm::JsonValue;

pub fn db_datetime_to_string(ts: NaiveDateTime) -> String {
    ts.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn map_db_error(err: anyhow::Error) -> tonic::Status {
    tonic::Status::internal(err.to_string())
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
