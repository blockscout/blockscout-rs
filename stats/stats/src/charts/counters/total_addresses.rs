use crate::{
    data_source::kinds::{
        local_db::DirectPointLocalDbChartSource,
        remote_db::{PullOne, RemoteDatabaseSource, StatementForOne},
    },
    ChartProperties, MissingDatePolicy, Named,
};
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct TotalAddressesStatement;

impl StatementForOne for TotalAddressesStatement {
    fn get_statement() -> Statement {
        Statement::from_string(
            DbBackend::Postgres,
            r#"
                SELECT
                    date, value
                FROM ( 
                    SELECT (
                        SELECT COUNT(*)::TEXT as value FROM addresses
                    ), (
                        SELECT MAX(b.timestamp)::DATE AS date
                        FROM blocks b
                        WHERE b.consensus = true
                    )
                ) as sub
            "#,
        )
    }
}

pub type TotalAddressesRemote =
    RemoteDatabaseSource<PullOne<TotalAddressesStatement, NaiveDate, String>>;

pub struct TotalAddressesProperties;

impl Named for TotalAddressesProperties {
    fn name() -> String {
                "totalAddresses".into()
            }
}

impl ChartProperties for TotalAddressesProperties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Counter
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
}

pub type TotalAddresses =
    DirectPointLocalDbChartSource<TotalAddressesRemote, TotalAddressesProperties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_counter;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_addresses() {
        simple_test_counter::<TotalAddresses>("update_total_addresses", "33", None).await;
    }
}
