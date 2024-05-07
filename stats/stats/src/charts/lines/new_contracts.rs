use crate::{
    charts::{
        data_source::{UpdateContext, UpdateParameters},
        db_interaction::chart_updaters::{ChartBatchUpdater, ChartUpdater},
    },
    DateValue, UpdateError,
};
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

#[derive(Default, Debug)]
pub struct NewContracts {}

impl ChartBatchUpdater for NewContracts {
    fn get_query(from: NaiveDate, to: NaiveDate) -> Statement {
        Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"SELECT day AS date, COUNT(*)::text AS value
                FROM (
                    SELECT 
                        DISTINCT ON (txns_plus_internal_txns.hash)
                        txns_plus_internal_txns.day
                    FROM (
                        SELECT
                            t.created_contract_address_hash AS hash,
                            b.timestamp::date AS day
                        FROM transactions t
                            JOIN blocks b ON b.hash = t.block_hash
                        WHERE
                            t.created_contract_address_hash NOTNULL AND
                            b.consensus = TRUE AND
                            b.timestamp != to_timestamp(0) AND
                            b.timestamp::date < $2 AND
                            b.timestamp::date >= $1
                        UNION
                        SELECT
                            it.created_contract_address_hash AS hash,
                            b.timestamp::date AS day
                        FROM internal_transactions it
                            JOIN blocks b ON b.hash = it.block_hash
                        WHERE
                            it.created_contract_address_hash NOTNULL AND
                            b.consensus = TRUE AND
                            b.timestamp != to_timestamp(0) AND
                            b.timestamp::date < $2 AND
                            b.timestamp::date >= $1
                    ) txns_plus_internal_txns
                ) sub
                GROUP BY sub.day;
                "#,
            vec![from.into(), to.into()],
        )
    }
}

impl crate::Chart for NewContracts {
    fn name() -> &'static str {
        "newContracts"
    }

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

impl ChartUpdater for NewContracts {
    async fn update_values(
        cx: &mut UpdateContext<UpdateParameters<'_>>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        Self::update_with_values(cx).await
    }
}

// #[cfg(test)]
// mod tests {
//     use super::NewContracts;
//     use crate::tests::simple_test::simple_test_chart;

//     #[tokio::test]
//     #[ignore = "needs database to run"]
//     async fn update_new_contracts() {
//         simple_test_chart::<NewContracts>(
//             "update_new_contracts",
//             vec![
//                 ("2022-11-09", "3"),
//                 ("2022-11-10", "6"),
//                 ("2022-11-11", "8"),
//                 ("2022-11-12", "2"),
//                 ("2022-12-01", "2"),
//                 ("2023-01-01", "1"),
//                 ("2023-02-01", "1"),
//             ],
//         )
//         .await;
//     }
// }
