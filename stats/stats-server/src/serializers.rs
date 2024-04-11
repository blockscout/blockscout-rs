use chrono::NaiveDate;
use stats::DateValue;
use stats_proto::blockscout::stats::v1::Point;

pub fn serialize_line_points(data: Vec<DateValue>) -> Vec<Point> {
    // let today: NaiveDate = serde_json::from_str(r#""2024-4-8""#).unwrap();
    data.into_iter()
        .map(|point| {
            // let is_approximate = point.date == today;
            Point {
                date: point.date.to_string(),
                value: point.value,
                is_approximate: false,
            }
        })
        .collect()
}
