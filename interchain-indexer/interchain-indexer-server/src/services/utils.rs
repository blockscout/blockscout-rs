use chrono::NaiveDateTime;

pub fn db_datetime_to_string(ts: NaiveDateTime) -> String {
    ts.and_utc()
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn map_db_error(err: anyhow::Error) -> tonic::Status {
    tonic::Status::internal(err.to_string())
}
