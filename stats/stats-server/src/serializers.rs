use stats::DateValue;
use stats_proto::blockscout::stats::v1::Point;

pub fn serialize_line_points(data: Vec<DateValue>) -> Vec<Point> {
    data.into_iter()
        .map(|point| Point {
            date: point.date.to_string(),
            value: point.value,
        })
        .collect()
}
