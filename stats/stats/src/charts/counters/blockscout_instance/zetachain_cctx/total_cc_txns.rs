use crate::{chart_prelude::*, lines::ZetachainCrossChainTxnsGrowth};

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalZetachainCrossChainTxns".into()
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
            .with_zetachain_cctx(ZetachainCctxIndexingStatus::IndexedHistoricalData)
    }
}

pub type TotalZetachainCrossChainTxns =
    DirectPointLocalDbChartSource<LastPoint<StripExt<ZetachainCrossChainTxnsGrowth>>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter_with_zetachain_cctx;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_zetachain_cross_chain_txns() {
        simple_test_counter_with_zetachain_cctx::<TotalZetachainCrossChainTxns>(
            "update_total_zetachain_cross_chain_txns",
            "3",
            None,
        )
        .await;
    }
}
