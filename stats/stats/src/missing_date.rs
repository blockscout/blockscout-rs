use crate::{DateValue, MissingDatePolicy};
use chrono::{Duration, NaiveDate};
use entity::chart_data;
use sea_orm::{prelude::*, sea_query::Expr, QueryOrder, QuerySelect};

pub async fn get_and_fill_chart(
    db: &DatabaseConnection,
    chart_id: i32,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    policy: MissingDatePolicy,
) -> Result<Vec<DateValue>, DbErr> {
    let mut data_request = chart_data::Entity::find()
        .select_only()
        .column(chart_data::Column::Date)
        .column(chart_data::Column::Value)
        .filter(chart_data::Column::ChartId.eq(chart_id))
        .order_by_asc(chart_data::Column::Date);

    if let Some(from) = from {
        let custom_where = Expr::cust_with_values::<sea_orm::sea_query::Value, _>(
            "date >= (SELECT COALESCE(MAX(date), '1900-01-01'::date) FROM chart_data WHERE chart_id = $1 AND date <= $2)",
            [chart_id.into(), from.into()],
        );
        QuerySelect::query(&mut data_request).cond_where(custom_where);
    }
    if let Some(to) = to {
        let custom_where = Expr::cust_with_values::<sea_orm::sea_query::Value, _>(
            "date <= (SELECT COALESCE(MIN(date), '9999-12-31'::date) FROM chart_data WHERE chart_id = $1 AND date >= $2)",
            [chart_id.into(), to.into()],
        );
        QuerySelect::query(&mut data_request).cond_where(custom_where);
    };

    let data_with_extra = data_request.into_model().all(db).await?;
    let data_filled = fill_missing_points(data_with_extra, policy, from, to);
    let data = filter_within_range(data_filled, from, to);
    Ok(data)
}

pub fn fill_missing_points(
    data: Vec<DateValue>,
    policy: MissingDatePolicy,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Vec<DateValue> {
    let from = vec![from.as_ref(), data.first().map(|v| &v.date)]
        .into_iter()
        .flatten()
        .min();
    let to = vec![to.as_ref(), data.last().map(|v| &v.date)]
        .into_iter()
        .flatten()
        .max();
    let (from, to) = match (from, to) {
        (Some(from), Some(to)) if from <= to => (from.to_owned(), to.to_owned()),
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

fn filter_within_range(
    data: Vec<DateValue>,
    maybe_from: Option<NaiveDate>,
    maybe_to: Option<NaiveDate>,
) -> Vec<DateValue> {
    let is_within_range = |v: &DateValue| -> bool {
        if let Some(from) = maybe_from {
            if v.date < from {
                return false;
            }
        }
        if let Some(to) = maybe_to {
            if v.date > to {
                return false;
            }
        }
        true
    };

    data.into_iter().filter(is_within_range).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;

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
            (
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-12", "12"),
                    v("2023-07-15", "12"),
                ],
                vec![
                    v("2023-07-10", "10"),
                    v("2023-07-11", "0"),
                    v("2023-07-12", "12"),
                    v("2023-07-13", "0"),
                    v("2023-07-14", "0"),
                    v("2023-07-15", "12"),
                ],
                Some(d("2023-07-12")),
                Some(d("2023-07-14")),
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
