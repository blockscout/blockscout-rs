use crate::{
    data_source::kinds::{
        data_manipulation::{last_point::LastPoint, map::StripExt},
        local_db::DirectPointLocalDbChartSource,
    },
    lines::OpStackOperationalTxnsGrowth,
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "opStackTotalOperationalTxns".into()
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

// sacrifice a bit of precision for much easier db load
// because filtering will do sequential scan over all transactions table
// (in contrast to `total_txns`, which uses an index).
pub type OpStackTotalOperationalTxns =
    DirectPointLocalDbChartSource<LastPoint<StripExt<OpStackOperationalTxnsGrowth>>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_op_stack_total_operational_txns() {
        simple_test_counter::<OpStackTotalOperationalTxns>(
            "update_op_stack_total_operational_txns",
            "55",
            None,
        )
        .await;
    }
}
