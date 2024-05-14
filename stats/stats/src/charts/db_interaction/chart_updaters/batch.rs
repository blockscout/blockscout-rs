use chrono::NaiveDate;
use sea_orm::Statement;

pub trait RemoteBatchQuery {
    fn get_query(from: NaiveDate, to: NaiveDate) -> Statement;
}
