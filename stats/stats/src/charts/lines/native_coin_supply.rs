use crate::{
    block_ranges,
    charts::{
        chart::{get_update_info, UpdateInfo},
        insert::{insert_data_many, DateValue},
    },
    metrics, Chart, UpdateError,
};
use async_trait::async_trait;
use chrono::Utc;
use entity::{native_coin_supply_data, sea_orm_active_enums::ChartType};
use sea_orm::{
    prelude::*, sea_query::OnConflict, ConnectionTrait, DbBackend, FromQueryResult, QuerySelect,
    Set, Statement,
};

#[derive(Default, Debug)]
pub struct NativeCoinSupply {}

impl NativeCoinSupply {
    async fn update_with_db_support(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        tracing::info!("start updating native_coin_supply");
        // default last row offset is 1, but we need to get actual last row
        let last_row_offset = Some(0);
        let update_info =
            get_update_info(self, db, blockscout, force_full, last_row_offset).await?;
        let _timer = metrics::CHART_FETCH_NEW_DATA_TIME
            .with_label_values(&[self.name()])
            .start_timer();

        let from_date = update_info.last_row.as_ref().map(|r| r.date);
        let to_date = Some(Utc::now().date_naive());
        let ranges = block_ranges::from_cache(db, blockscout, from_date, to_date).await?;
        // if there is no last_row, it means we start calculate from begginning,
        // therefore local database should be empty
        if from_date.is_none() {
            let deleted = native_coin_supply_data::Entity::delete_many()
                .exec(db)
                .await
                .map_err(UpdateError::StatsDB)?;
            if deleted.rows_affected > 0 {
                tracing::warn!(rows =? deleted.rows_affected, "deleted several rows from native_coin_supply_data");
            }
        }
        Self::update_with_ranges(db, blockscout, &update_info, ranges).await
    }

    async fn update_with_ranges(
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        update_info: &UpdateInfo,
        ranges: Vec<entity::block_ranges::Model>,
    ) -> Result<(), UpdateError> {
        let n = ranges.len();
        for (i, range) in ranges.into_iter().enumerate() {
            tracing::info!(range =? range, "calculating {}/{} step of native coin supply", i+1, n);
            let changed_balances = Self::changed_balances_for_range(blockscout, &range)
                .await?
                .into_iter()
                .map(|data| native_coin_supply_data::ActiveModel {
                    address: Set(data.address),
                    balance: Set(data.balance),
                    date: Set(data.date),
                });
            if changed_balances.len() > 0 {
                insert_balances(db, changed_balances).await?;
            }
            let value = count_current_balances(db).await?;
            let data = DateValue {
                date: range.date,
                value: value.to_string(),
            }
            .active_model(update_info.chart_id, Some(update_info.min_blockscout_block));
            insert_data_many(db, vec![data])
                .await
                .map_err(UpdateError::StatsDB)?;
        }
        Ok(())
    }

    async fn changed_balances_for_range(
        blockscout: &DatabaseConnection,
        range: &entity::block_ranges::Model,
    ) -> Result<Vec<native_coin_supply_data::Model>, UpdateError> {
        let sql = format!(
            r#"SELECT
                DISTINCT on (address_hash) address_hash as address, 
                value as balance, 
                date('{}') as date
            FROM address_coin_balances
            WHERE 
                $1 <= block_number AND block_number <= $2
            ORDER BY address_hash, block_number DESC;"#,
            range.date
        );

        native_coin_supply_data::Entity::find()
            .from_raw_sql(Statement::from_sql_and_values(
                DbBackend::Postgres,
                &sql,
                vec![range.from_number.into(), range.to_number.into()],
            ))
            .all(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)
    }
}

#[derive(Debug, FromQueryResult)]
struct Sum {
    sum: Option<Decimal>,
}

async fn count_current_balances<C>(db: &C) -> Result<Decimal, UpdateError>
where
    C: ConnectionTrait,
{
    let result = native_coin_supply_data::Entity::find()
        .select_only()
        .column_as(native_coin_supply_data::Column::Balance.sum(), "sum")
        .into_model::<Sum>()
        .one(db)
        .await
        .map_err(UpdateError::StatsDB)?;
    Ok(result
        .map(|s| s.sum.unwrap_or_default())
        .unwrap_or_default())
}

async fn insert_balances<C, I>(db: &C, balances: I) -> Result<(), UpdateError>
where
    C: ConnectionTrait,
    I: IntoIterator<Item = native_coin_supply_data::ActiveModel>,
{
    native_coin_supply_data::Entity::insert_many(balances)
        .on_conflict(
            OnConflict::column(native_coin_supply_data::Column::Address)
                .update_columns([
                    native_coin_supply_data::Column::Balance,
                    native_coin_supply_data::Column::Date,
                ])
                .to_owned(),
        )
        .exec(db)
        .await
        .map_err(UpdateError::StatsDB)?;
    Ok(())
}

#[async_trait]
impl crate::Chart for NativeCoinSupply {
    fn name(&self) -> &str {
        "nativeCoinSupply"
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
        self.update_with_db_support(db, blockscout, force_full)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coin_supply() {
        let chart = NativeCoinSupply::default();

        // TODO: add mocked data to blockscout address_coin_balances table
        simple_test_chart(
            "update_native_coin_supply",
            chart,
            vec![
                ("2022-11-09", "0"),
                ("2022-11-10", "0"),
                ("2022-11-11", "0"),
                ("2022-11-12", "0"),
                ("2022-12-01", "0"),
                ("2023-01-01", "0"),
                ("2023-02-01", "0"),
                ("2023-03-01", "0"),
            ],
        )
        .await;
    }
}
