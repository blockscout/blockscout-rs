use chrono::{Duration, NaiveDate};
use stats::{DateValue, MissingDatePolicy};
use stats_proto::blockscout::stats::v1::Point;

pub fn serialize_line_points(
    data: Vec<DateValue>,
    missing_date_policy: MissingDatePolicy,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Vec<Point> {
    let data = fill_missing_points(data, missing_date_policy, from, to);
    let serialized_chart: Vec<_> = data
        .into_iter()
        .map(|point| Point {
            date: point.date.to_string(),
            value: point.value,
        })
        .collect();

    serialized_chart
}

fn fill_missing_points(
    data: Vec<DateValue>,
    policy: MissingDatePolicy,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Vec<DateValue> {
    let from = from.or(data.first().map(|v| v.date));
    let to = to.or(data.last().map(|v| v.date));
    let (from, to) = match (from, to) {
        (Some(from), Some(to)) if from <= to => (from, to),
        _ => return data,
    };

    match policy {
        MissingDatePolicy::FillZero => fill_zeros(data, from, to),
        MissingDatePolicy::FillPrevious => fill_previous(data, from, to),
    }
}

fn fill_zeros(data: Vec<DateValue>, from: NaiveDate, to: NaiveDate) -> Vec<DateValue> {
    let n = (to - from).num_days() as usize;
    let mut new_data: Vec<DateValue> = Vec::with_capacity(n);

    let mut current_date = from;
    let mut i = 0;
    while current_date <= to {
        let maybe_value_exists = data.get(i).filter(|&v| v.date.eq(&current_date));

        let value = match maybe_value_exists {
            Some(value) => {
                i += 1;
                value.clone()
            }
            None => DateValue::zero(current_date),
        };
        new_data.push(value);
        current_date += Duration::days(1);
    }

    new_data
}

fn fill_previous(data: Vec<DateValue>, from: NaiveDate, to: NaiveDate) -> Vec<DateValue> {
    let n = (to - from).num_days() as usize;
    let mut new_data: Vec<DateValue> = Vec::with_capacity(n);
    let mut current_date = from;
    let mut i = 0;
    while current_date <= to {
        let maybe_value_exists = data.get(i).filter(|&v| v.date.eq(&current_date));
        let value = match maybe_value_exists {
            Some(value) => {
                i += 1;
                value.clone()
            }
            None => new_data
                .last()
                .map(|value| DateValue {
                    date: current_date,
                    value: value.value.clone(),
                })
                .unwrap_or_else(|| DateValue::zero(current_date)),
        };
        new_data.push(value);
        current_date += Duration::days(1);
    }
    new_data
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;
    use stats::DateValue;

    fn d(date: &str) -> NaiveDate {
        date.parse().unwrap()
    }
    fn v(date: &str, value: &str) -> DateValue {
        DateValue {
            date: d(date),
            value: value.to_string(),
        }
    }

    #[test]
    fn fill_zeros_works() {
        for (data, expected, from, to) in [
            (vec![], vec![], None, None),
            (vec![], vec![], Some(d("2100-01-01")), None),
            (
                vec![],
                vec![v("2100-01-01", "0")],
                Some(d("2100-01-01")),
                Some(d("2100-01-01")),
            ),
            (
                vec![v("2022-01-01", "01")],
                vec![v("2022-01-01", "01")],
                Some(d("2022-01-01")),
                Some(d("2022-01-01")),
            ),
            (
                vec![
                    v("2022-08-20", "20"),
                    v("2022-08-22", "22"),
                    v("2022-08-23", "23"),
                    v("2022-08-25", "25"),
                ],
                vec![
                    v("2022-08-18", "0"),
                    v("2022-08-19", "0"),
                    v("2022-08-20", "20"),
                    v("2022-08-21", "0"),
                    v("2022-08-22", "22"),
                    v("2022-08-23", "23"),
                    v("2022-08-24", "0"),
                    v("2022-08-25", "25"),
                    v("2022-08-26", "0"),
                    v("2022-08-27", "0"),
                ],
                Some(d("2022-08-18")),
                Some(d("2022-08-27")),
            ),
        ] {
            let actual = fill_missing_points(data, MissingDatePolicy::FillZero, from, to);
            assert_eq!(expected, actual)
        }
    }

    #[test]
    fn fill_previous_works() {
        for (data, expected, from, to) in [
            (vec![], vec![], None, None),
            (vec![], vec![], Some(d("2100-01-01")), None),
            (
                vec![],
                vec![v("2100-01-01", "0")],
                Some(d("2100-01-01")),
                Some(d("2100-01-01")),
            ),
            (
                vec![v("2022-01-01", "01")],
                vec![v("2022-01-01", "01")],
                Some(d("2022-01-01")),
                Some(d("2022-01-01")),
            ),
            (
                vec![
                    v("2022-08-20", "20"),
                    v("2022-08-22", "22"),
                    v("2022-08-23", "23"),
                    v("2022-08-25", "25"),
                ],
                vec![
                    v("2022-08-18", "0"),
                    v("2022-08-19", "0"),
                    v("2022-08-20", "20"),
                    v("2022-08-21", "20"),
                    v("2022-08-22", "22"),
                    v("2022-08-23", "23"),
                    v("2022-08-24", "23"),
                    v("2022-08-25", "25"),
                    v("2022-08-26", "25"),
                    v("2022-08-27", "25"),
                ],
                Some(d("2022-08-18")),
                Some(d("2022-08-27")),
            ),
        ] {
            let actual = fill_missing_points(data, MissingDatePolicy::FillPrevious, from, to);
            assert_eq!(expected, actual);
        }
    }
}
