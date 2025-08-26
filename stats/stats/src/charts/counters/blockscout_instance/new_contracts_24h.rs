use crate::chart_prelude::*;

use blockscout_db::entity::transactions;

pub struct NewContracts24hStatement;
impl_db_choice!(NewContracts24hStatement, UsePrimaryDB);

impl StatementFromUpdateTime for NewContracts24hStatement {
    fn get_statement(
        update_time: DateTime<Utc>,
        _completed_migrations: &IndexerMigrations,
    ) -> sea_orm::Statement {
        transactions::Entity::find()
            .select_only()
            .filter(transactions::Column::Status.eq(1))
            .filter(interval_24h_filter(
                transactions::Column::CreatedContractCodeIndexedAt.into_simple_expr(),
                update_time,
            ))
            .expr_as(Func::count(Asterisk.into_column_ref()), "value")
            .build(DbBackend::Postgres)
    }
}

pub type NewContracts24hRemote =
    RemoteDatabaseSource<PullOneNowValue<NewContracts24hStatement, NaiveDate, i64>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newContracts24h".into()
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

/// Does not include contracts from internal txns
/// (for performance reasons)
pub type NewContracts24h =
    DirectPointLocalDbChartSource<MapToString<NewContracts24hRemote>, Properties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{point_construction::dt, simple_test::simple_test_counter};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_contracts_24h() {
        simple_test_counter::<NewContracts24h>(
            "update_new_contracts_24h",
            "8",
            Some(dt("2022-11-11T16:30:00")),
        )
        .await;
    }
}
