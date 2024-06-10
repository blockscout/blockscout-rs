use crate::data_source::kinds::updateable_chart::clone::point::ClonePointChartWrapper;

mod _inner {
    use crate::{
        data_source::kinds::{
            remote_db::{PullOne, RemoteDatabaseSource, StatementForOne},
            updateable_chart::clone::point::ClonePointChart,
        },
        Chart, DateValueString, Named,
    };
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
        RemoteDatabaseSource<PullOne<TotalAddressesStatement, DateValueString>>;

    pub struct TotalAddressesInner;

    impl Named for TotalAddressesInner {
        const NAME: &'static str = "totalAddresses";
    }

    impl Chart for TotalAddressesInner {
        fn chart_type() -> ChartType {
            ChartType::Counter
        }
    }

    impl ClonePointChart for TotalAddressesInner {
        type Dependency = TotalAddressesRemote;
    }
}

pub type TotalAddresses = ClonePointChartWrapper<_inner::TotalAddressesInner>;

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
