use crate::{chart_prelude::*, lines::NewBlocksStatement};

use super::{CalculateOperationalTxns, yesterday_txns::YesterdayTxnsInt};

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
        IndexingStatus::LEAST_RESTRICTIVE
    }
}

pub type ArbitrumYesterdayOperationalTxns = DirectPointLocalDbChartSource<
    Map<(YesterdayBlocksRemoteInt, YesterdayTxnsInt), CalculateOperationalTxns<Properties>>,
    Properties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_arbitrum_yesterday_operational_txns() {
        // 14 - 3 (txns - blocks)
        simple_test_counter::<ArbitrumYesterdayOperationalTxns>(
            "update_arbitrum_yesterday_operational_txns",
            "11",
            Some(dt("2022-11-11T00:00:00")),
        )
        .await;
    }
}
