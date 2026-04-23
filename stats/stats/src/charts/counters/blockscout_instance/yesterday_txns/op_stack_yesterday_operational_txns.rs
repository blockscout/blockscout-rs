use crate::{chart_prelude::*, types::new_txns::ExtractOpStackTxns};

use super::YesterdayTxnsCombinedRemote;

pub type OpStackYesterdayOperationalTxnsRemote =
    Map<YesterdayTxnsCombinedRemote, ExtractOpStackTxns>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "opStackYesterdayOperationalTxns".into()
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

pub type OpStackYesterdayOperationalTxns =
    DirectPointLocalDbChartSource<OpStackYesterdayOperationalTxnsRemote, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_op_stack_yesterday_operational_txns() {
        simple_test_counter::<OpStackYesterdayOperationalTxns>(
            "update_op_stack_yesterday_operational_txns",
            "1",
            Some(dt("2023-03-02T00:00:00")),
        )
        .await;
    }
}
