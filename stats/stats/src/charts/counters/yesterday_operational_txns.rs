use crate::{
    data_source::{
        kinds::{
            data_manipulation::map::{Map, MapParseTo},
            local_db::DirectPointLocalDbChartSource,
            remote_db::{RemoteDatabaseSource, RemoteQueryBehaviour},
        },
        UpdateContext,
    },
    lines::NewBlocksStatement,
    range::UniversalRange,
    types::TimespanValue,
    ChartError, ChartProperties, IndexingStatus, MissingDatePolicy, Named,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;

use super::{
    yesterday_txns::{query_yesterday_data, YesterdayTxnsInt},
    CalculateOperationalTxns,
};

pub struct YesterdayBlocksQuery;

impl RemoteQueryBehaviour for YesterdayBlocksQuery {
    type Output = TimespanValue<NaiveDate, String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let today = cx.time.date_naive();
        query_yesterday_data::<NewBlocksStatement>(cx, today).await
    }
}

pub type YesterdayBlocksRemote = RemoteDatabaseSource<YesterdayBlocksQuery>;

// should only be used in this chart for query efficiency.
// because is not directly stored in local DB.
pub type YesterdayBlocksRemoteInt = MapParseTo<YesterdayBlocksRemote, i64>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "yesterdayOperationalTxns".into()
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

    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::NoneIndexed
    }
}

pub type YesterdayOperationalTxns = DirectPointLocalDbChartSource<
    Map<(YesterdayBlocksRemoteInt, YesterdayTxnsInt), CalculateOperationalTxns<Properties>>,
    Properties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_yesterday_operational_txns() {
        // 14 - 3 (txns - blocks)
        simple_test_counter::<YesterdayOperationalTxns>(
            "update_yesterday_operational_txns",
            "11",
            Some(dt("2022-11-11T00:00:00")),
        )
        .await;
    }
}
