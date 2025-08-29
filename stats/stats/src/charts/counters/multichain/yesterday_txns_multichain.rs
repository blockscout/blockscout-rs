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

    // ("2023-02-04", 1, 10, 46, 170),
    // ("2023-02-04", 2, 20, 55, 300),
    // ("2023-02-04", 3, 30, 109, 450),
    // ("2023-02-03", 1, 4, 36, 160),
    // ("2023-02-03", 2, 7, 35, 290),
    // ("2023-02-03", 3, 38, 79, 422),
    // ("2023-02-02", 1, 18, 32, 155),
    // ("2023-02-02", 2, 3, 28, 250),
    // ("2023-02-02", 3, 4, 41, 420),
    // ("2023-01-01", 1, 3, 14, 150),
    // ("2023-01-01", 2, 3, 25, 250),
    // ("2023-01-01", 3, 4, 37, 350),
    // ("2022-12-28", 1, 11, 11, 111),
    // ("2022-12-28", 2, 22, 22, 222),
    // ("2022-12-28", 3, 33, 33, 333),
}
