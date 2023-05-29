use crate::{
    charts::{
        insert::DateValue,
        updater::{split_update, ChartPartialUpdater},
    },
    UpdateError,
};
use async_trait::async_trait;
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

#[derive(Default, Debug)]
pub struct NewContracts {}

#[async_trait]
impl ChartPartialUpdater for NewContracts {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
        last_row: Option<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let query_maker = |from_: NaiveDate, to_: NaiveDate| {
            Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"select day as date, count(*)::text as value
                from (
                    select 
                        distinct txns_plus_internal_txns.hash,
                        txns_plus_internal_txns.day
                    from (
                        select
                            t.created_contract_address_hash as hash,
                            b.timestamp::date as day
                        FROM transactions t
                            JOIN blocks b ON b.hash = t.block_hash
                        where
                            t.created_contract_address_hash notnull and
                            b.consensus = true and
                            b.timestamp::date < $2 and
                            b.timestamp::date >= $1
                        union
                        select
                            it.created_contract_address_hash as hash,
                            b.timestamp::date as day
                        FROM internal_transactions it
                            JOIN blocks b ON b.hash = it.block_hash
                        where
                            it.created_contract_address_hash notnull and
                            b.consensus = true and
                            b.timestamp::date < $2 and
                            b.timestamp::date >= $1
                    ) txns_plus_internal_txns
                ) sub
                group by sub.day;
                "#,
                vec![from_.into(), to_.into()],
            )
        };

        split_update(blockscout, last_row, query_maker).await
    }
}

#[async_trait]
impl crate::Chart for NewContracts {
    fn name(&self) -> &str {
        "newContracts"
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Line
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        self.update_with_values(db, blockscout, force_full).await
    }
}

#[cfg(test)]
mod tests {
    use super::NewContracts;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_contracts() {
        let chart = NewContracts::default();
        simple_test_chart(
            "update_new_contracts",
            chart,
            vec![
                ("2022-11-09", "3"),
                ("2022-11-10", "6"),
                ("2022-11-11", "8"),
                ("2022-11-12", "2"),
                ("2022-12-01", "2"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }
}
