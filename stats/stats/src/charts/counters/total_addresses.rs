use crate::{
    charts::db_interaction::read::query_estimated_table_rows,
    data_source::{
        kinds::{
            local_db::{parameters::ValueEstimation, DirectPointLocalDbChartSourceWithEstimate},
            remote_db::{PullOne, RemoteDatabaseSource, StatementForOne},
        },
        types::BlockscoutMigrations,
    },
    types::timespans::DateValue,
    ChartError, ChartProperties, IndexingStatus, MissingDatePolicy, Named,
};
use blockscout_db::entity::addresses;
use chrono::{NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DatabaseConnection, DbBackend, EntityName, Statement};

pub struct TotalAddressesStatement;

impl StatementForOne for TotalAddressesStatement {
    fn get_statement(_: &BlockscoutMigrations) -> Statement {
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

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "totalAddresses".into()
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
        IndexingStatus::NoneIndexed
    }
}

pub struct TotalAddressesEstimation;

impl ValueEstimation for TotalAddressesEstimation {
    async fn estimate(blockscout: &DatabaseConnection) -> Result<DateValue<String>, ChartError> {
        // `now()` is more relevant when taken right before the query rather than
        // `cx.time` measured a bit earlier.
        let now = Utc::now();
        let value = query_estimated_table_rows(blockscout, addresses::Entity.table_name())
            .await
            .map_err(ChartError::BlockscoutDB)?
            .map(|n| u64::try_from(n).unwrap_or(0))
            .unwrap_or(0);
        Ok(DateValue {
            timespan: now.date_naive(),
            value: value.to_string(),
        })
    }
}

pub type TotalAddresses = DirectPointLocalDbChartSourceWithEstimate<
    TotalAddressesRemote,
    TotalAddressesEstimation,
    Properties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::{simple_test_counter, test_counter_fallback};

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_addresses() {
        simple_test_counter::<TotalAddresses>("update_total_addresses", "33", None).await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn total_addresses_fallback() {
        test_counter_fallback::<TotalAddresses>("total_addresses_fallback").await;
    }
}
