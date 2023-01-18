use chrono::NaiveDate;
use sea_orm::FromQueryResult;

#[derive(Debug, FromQueryResult)]
pub struct OnlyDate {
    pub date: NaiveDate,
}
