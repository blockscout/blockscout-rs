use chrono::NaiveDate;
use stats::types::ExtendedTimespanValue;
use stats_proto::blockscout::stats::v1::Point;

pub fn serialize_line_points(data: Vec<ExtendedTimespanValue<NaiveDate, String>>) -> Vec<Point> {
    data.into_iter()
        .map(|point| Point {
            date: point.date.to_string(),
            value: point.value,
            is_approximate: point.is_approximate,
        })
        .collect()
}
