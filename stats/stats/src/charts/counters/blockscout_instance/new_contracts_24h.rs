// SPDX-License-Identifier: LicenseRef-Blockscout

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
    use crate::tests::{normalize_sql, point_construction::dt, simple_test::simple_test_counter};

    /// The 24h bounds must compare the bare column against constants, so that
    /// `transactions_created_contract_code_indexed_at_index` stays usable.
    #[test]
    fn statement_is_correct() {
        let actual = NewContracts24hStatement::get_statement(
            dt("2025-01-02T00:00:00").and_utc(),
            &IndexerMigrations::latest(),
        );

        let expected = r#"
            SELECT COUNT(*) AS "value"
            FROM "transactions"
            WHERE "transactions"."status" = 1
                AND ("transactions"."created_contract_code_indexed_at" >= '2025-01-01 00:00:00.000000 +00:00'
                AND "transactions"."created_contract_code_indexed_at" <= '2025-01-02 00:00:00.000000 +00:00')
        "#;
        assert_eq!(normalize_sql(expected), normalize_sql(&actual.to_string()))
    }

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
