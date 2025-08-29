use crate::{chart_prelude::*, lines::multichain::new_txns_multichain::NewTxnsMultichainStatement};

pub struct YesterdayTxnsMultichainQuery;

impl RemoteQueryBehaviour for YesterdayTxnsMultichainQuery {
    type Output = DateValue<String>;

    async fn query_data(
        cx: &UpdateContext<'_>,
        _range: UniversalRange<DateTime<Utc>>,
    ) -> Result<Self::Output, ChartError> {
        let today = cx.time.date_naive();
        query_yesterday_data::<NewTxnsMultichainStatement>(cx, today).await
    }
}

pub type YesterdayTxnsMultichainRemote = RemoteDatabaseSource<YesterdayTxnsMultichainQuery>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "yesterdayTxnsMultichain".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }

    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::LEAST_RESTRICTIVE
    }
}

pub type YesterdayTxnsMultichain =
    DirectPointLocalDbChartSource<MapToString<YesterdayTxnsMultichainRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter_multichain};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_yesterday_txns_multichain() {
        simple_test_counter_multichain::<YesterdayTxnsMultichain>(
            "update_total_multichain_txns",
            "0",
            Some(dt("2023-02-06T00:00:00")),
        )
        .await;

        simple_test_counter_multichain::<YesterdayTxnsMultichain>(
            "update_total_multichain_txns",
            "60",
            Some(dt("2023-02-05T00:00:00")),
        )
        .await;

        simple_test_counter_multichain::<YesterdayTxnsMultichain>(
            "update_total_multichain_txns",
            "49",
            Some(dt("2023-02-04T00:00:00")),
        )
        .await;
    }
}
