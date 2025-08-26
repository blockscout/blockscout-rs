//! New optimism stack operational transactions for the last N days
//! (usually 30).
//!
//! Basically a [super::NewTxnsWindow] but for op stack operational txns

use crate::{chart_prelude::*, types::new_txns::ExtractOpStackTxns};

use super::NewTxnsWindowCombinedRemote;

pub type OpStackNewOperationalTxnsWindowRemote =
    Map<NewTxnsWindowCombinedRemote, ExtractOpStackTxns>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "opStackNewOperationalTxnsWindow".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn indexing_status_requirement() -> IndexingStatus {
        IndexingStatus::LEAST_RESTRICTIVE
    }
}

pub type OpStackNewOperationalTxnsWindow = LocalDbChartSource<
    OpStackNewOperationalTxnsWindowRemote,
    (),
    DefaultCreate<Properties>,
    ClearAllAndPassVec<
        OpStackNewOperationalTxnsWindowRemote,
        DefaultQueryVec<Properties>,
        Properties,
    >,
    DefaultQueryVec<Properties>,
    Properties,
>;

#[cfg(test)]
mod tests {

    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_op_stack_new_operational_txns_window() {
        simple_test_chart::<OpStackNewOperationalTxnsWindow>(
            "update_op_stack_new_operational_txns_window",
            vec![("2023-02-01", "5")],
        )
        .await;
    }
}
