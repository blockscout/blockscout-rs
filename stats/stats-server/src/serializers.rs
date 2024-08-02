use chrono::NaiveDate;
use stats::{
    exclusive_datetime_range_to_inclusive,
    types::{ExtendedTimespanValue, Timespan},
};
use stats_proto::blockscout::stats::v1::Point;

pub fn serialize_line_points(data: Vec<ExtendedTimespanValue<NaiveDate, String>>) -> Vec<Point> {
    data.into_iter()
        .map(|point| {
            let time_range =
                exclusive_datetime_range_to_inclusive(point.timespan.into_time_range());
            let date_range = { time_range.start().date_naive()..=time_range.end().date_naive() };
            Point {
                date: date_range.start().to_string(),
                date_to: date_range.end().to_string(),
                value: point.value,
                is_approximate: point.is_approximate,
            }
        })
        .collect()
}
