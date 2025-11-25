use chrono::NaiveDateTime;

const NANOS_PER_SEC: i64 = 1_000_000_000;

// Serialize NaiveDateTime to 8 bytes (nanoseconds since Unix epoch)
// This approach support dates up to 2262 year
pub fn naive_datetime_to_bytes(dt: NaiveDateTime) -> anyhow::Result<[u8; 8]> {
    let secs = dt.and_utc().timestamp(); // i64 seconds since Unix epoch
    let nanos = dt.and_utc().timestamp_subsec_nanos(); // u32 nanoseconds

    // Convert to i64 nanoseconds since Unix epoch
    let total_nanos = secs
        .checked_mul(NANOS_PER_SEC)
        .and_then(|base| base.checked_add(nanos as i64))
        .ok_or_else(|| anyhow::anyhow!("NaiveDateTime is out of supported range for encoding"))?;

    Ok(total_nanos.to_be_bytes())
}

// Deserialize NaiveDateTime
pub fn bytes_to_naive_datetime(bytes: [u8; 8]) -> anyhow::Result<NaiveDateTime> {
    let total_nanos = i64::from_be_bytes(bytes);

    let secs = total_nanos
        .checked_div(NANOS_PER_SEC)
        .ok_or_else(|| anyhow::anyhow!("Overflow while converting nanos to seconds"))?;

    let nanos = (total_nanos
        .checked_rem(NANOS_PER_SEC)
        .ok_or_else(|| anyhow::anyhow!("Overflow while computing nanos remainder"))?)
        as u32;

    let dt = chrono::DateTime::from_timestamp(secs, nanos)
        .ok_or_else(|| anyhow::anyhow!("Failed to construct DateTime from timestamp"))?
        .naive_utc();

    Ok(dt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rstest::rstest;

    #[rstest]
    #[case(2025, 11, 25, 12, 0, 0, 123_456_789)]
    #[case(2025, 1, 1, 0, 0, 0, 0)]
    #[case(2025, 1, 1, 23, 59, 59, 999_999_999)]
    #[case(2262, 4, 11, 23, 47, 16, 854_775_807)] // max supported timestamp
    #[case(1970, 1, 1, 0, 0, 0, 0)]
    fn test_naive_datetime_to_bytes_round_trip(
        #[case] year: i32,
        #[case] month: u32,
        #[case] day: u32,
        #[case] hours: u32,
        #[case] minutes: u32,
        #[case] seconds: u32,
        #[case] nanosecond: u32,
    ) {
        let dt = NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_nano_opt(hours, minutes, seconds, nanosecond)
            .unwrap();
        let bytes = naive_datetime_to_bytes(dt).unwrap();
        let restored = bytes_to_naive_datetime(bytes).unwrap();
        assert_eq!(dt, restored);
    }
}
