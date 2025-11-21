use multichain_aggregator_entity::interop_messages;
use sea_orm::Condition;

use crate::chart_prelude::*;

pub struct TotalInteropMessagesStatement;
impl_db_choice!(TotalInteropMessagesStatement, UsePrimaryDB);

impl StatementFromUpdateTime for TotalInteropMessagesStatement {
    fn get_statement_with_context(cx: &UpdateContext<'_>) -> sea_orm::Statement {

        let mut query = interop_messages::Entity::find()
            .select_only()
            .filter(interop_messages::Column::Timestamp.lte(cx.time));

        if let Some(filter) = &cx.multichain_filter {
            if !filter.is_empty() {
                let chain_ids: Vec<i64> = filter.iter().map(|&id| id as i64).collect();
                query = query.filter(
                    Condition::any()
                        .add(interop_messages::Column::InitChainId.is_in(chain_ids.clone()))
                        .add(interop_messages::Column::RelayChainId.is_in(chain_ids)),
                );
            }
        }

        query
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .build(DbBackend::Postgres)
    }
}

pub type TotalInteropMessagesRemote =
    RemoteDatabaseSource<PullOneNowValue<TotalInteropMessagesStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalInteropMessages".into()
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

pub type TotalInteropMessages =
    DirectPointLocalDbChartSource<MapToString<TotalInteropMessagesRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter_multichain};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_interop_messages() {
        simple_test_counter_multichain::<TotalInteropMessages>(
            "update_total_interop_messages",
            "6",
            None,
            None,
        )
        .await;

        simple_test_counter_multichain::<TotalInteropMessages>(
            "update_total_interop_messages",
            "4",
            None,
            Some(vec![2, 3]),
        )
        .await;

        simple_test_counter_multichain::<TotalInteropMessages>(
            "update_total_interop_messages",
            "4",
            Some(dt("2022-11-15T12:00:00")),
            None,
        )
        .await;
    }
}
