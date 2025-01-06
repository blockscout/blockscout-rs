use crate::{
    data_source::{
        kinds::{
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour, StatementFromRange},
        },
        UpdateContext,
    },
    lines::NewTxnsStatement,
    range::UniversalRange,
    types::TimespanValue,
    utils::day_start,
    ChartError, ChartProperties, MissingDatePolicy, Named,
};
use chrono::{DateTime, Days, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::FromQueryResult;

pub struct YesterdayTxnsQuery;

impl RemoteQueryBehaviour for YesterdayTxnsQuery {
    type Output = TimespanValue<NaiveDate, String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let today = cx.time.date_naive();
        let yesterday = today
            .checked_sub_days(Days::new(1))
            .ok_or(ChartError::Internal(
                "Update time is incorrect: ~ minimum possible date".into(),
            ))?;
        let yesterday_range = day_start(&yesterday)..day_start(&today);
        let query = NewTxnsStatement::get_statement(
            Some(yesterday_range),
            &cx.blockscout_applied_migrations,
        );
        let data = Self::Output::find_by_statement(query)
            .one(cx.blockscout)
            .await
            .map_err(ChartError::BlockscoutDB)?
            // no transactions for yesterday
            .unwrap_or(TimespanValue::with_zero_value(yesterday));
        Ok(data)
    }
}

pub type YesterdayTxnsRemote = RemoteDatabaseSource<YesterdayTxnsQuery>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "yesterdayTxns".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type YesterdayTxns = DirectPointLocalDbChartSource<YesterdayTxnsRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_yesterday_txns_1() {
        simple_test_counter::<YesterdayTxns>("update_yesterday_txns_1", "0", None).await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_yesterday_txns_2() {
        simple_test_counter::<YesterdayTxns>(
            "update_yesterday_txns_2",
            "12",
            Some(dt("2022-11-11T00:00:00")),
        )
        .await;
    }
}
