use crate::{
    data_source::{
        kinds::{
            data_manipulation::map::{Map, MapFunction},
            local_db::DirectPointLocalDbChartSource,
        },
        DataSource,
    },
    types::TimespanValue,
    ChartProperties, MissingDatePolicy, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use tracing::warn;

use super::{TotalBlocksInt, TotalTxnsInt};

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalOperationalTxns".into()
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

pub struct Calculate;

type Input = (
    <TotalBlocksInt as DataSource>::Output,
    <TotalTxnsInt as DataSource>::Output,
);

impl MapFunction<Input> for Calculate {
    type Output = TimespanValue<NaiveDate, String>;

    fn function(inner_data: Input) -> Result<Self::Output, crate::UpdateError> {
        let (total_blocks_data, total_txns_data) = inner_data;
        if total_blocks_data.timespan != total_txns_data.timespan {
            warn!("timespans for total blocks and total transactions do not match when calculating {}", Properties::name());
        }
        let date = total_blocks_data.timespan;
        let value = total_txns_data
            .value
            .saturating_sub(total_blocks_data.value);
        Ok(TimespanValue {
            timespan: date,
            value: value.to_string(),
        })
    }
}

pub type TotalOperationalTxns =
    DirectPointLocalDbChartSource<Map<(TotalBlocksInt, TotalTxnsInt), Calculate>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_operational_txns() {
        // 47 - 13 (txns - blocks)
        simple_test_counter::<TotalOperationalTxns>("update_total_operational_txns", "34", None)
            .await;
    }
}
