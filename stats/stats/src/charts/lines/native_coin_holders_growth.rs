use crate::{
    charts::{
        create_chart,
        db_interaction::{
            insert::insert_data_many,
            types::DateValue,
            updater::{get_last_row, get_min_block_blockscout},
        },
        find_chart,
    },
    Chart, MissingDatePolicy, UpdateError,
};
use async_trait::async_trait;
use blockscout_db::entity::address_coin_balances_daily;
use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use itertools::Itertools;
use migration::OnConflict;
use sea_orm::{
    prelude::*, ConnectionTrait, FromQueryResult, QueryOrder, QuerySelect, Set, Statement,
    TransactionTrait,
};
use std::collections::{BTreeMap, HashSet};

#[derive(Default, Debug)]
pub struct NativeCoinHoldersGrowth {}

mod db_address_balances {
    use sea_orm::prelude::*;

    // `nchg` is native_coin_holders_growth
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "support_nchg_addresses_balances")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub address: Vec<u8>,
        pub balance: Decimal,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

#[async_trait]
impl crate::Chart for NativeCoinHoldersGrowth {
    fn name(&self) -> &str {
        "nativeCoinHoldersGrowth"
    }
    fn chart_type(&self) -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy(&self) -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
    fn approximate_trailing_values(&self) -> u64 {
        // support table contains information of actual last day
        0
    }

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        self.create_support_table(db).await?;
        create_chart(db, self.name().into(), self.chart_type()).await
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        let chart_id = find_chart(db, self.name())
            .await
            .map_err(UpdateError::StatsDB)?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let min_blockscout_block = get_min_block_blockscout(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        let offset = Some(self.approximate_trailing_values());
        let last_row =
            get_last_row(self, chart_id, min_blockscout_block, db, force_full, offset).await?;
        self.update_sequentially_with_support_table(
            db,
            blockscout,
            last_row,
            chart_id,
            min_blockscout_block,
        )
        .await?;
        Ok(())
    }
}

// TODO: move common logic to new updater trait
impl NativeCoinHoldersGrowth {
    pub async fn update_sequentially_with_support_table(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        last_row: Option<DateValue>,
        chart_id: i32,
        min_blockscout_block: i64,
    ) -> Result<(), UpdateError> {
        tracing::info!("start sequential update for chart {}", self.name());
        let all_days = match last_row {
            Some(last_row) => get_unique_ordered_days(blockscout, Some(last_row.date))
                .await
                .map_err(UpdateError::BlockscoutDB)?,
            None => {
                self.clear_support_table(db)
                    .await
                    .map_err(UpdateError::BlockscoutDB)?;
                get_unique_ordered_days(blockscout, None)
                    .await
                    .map_err(UpdateError::BlockscoutDB)?
            }
        };

        for days in all_days.chunks(self.step_duration_days()) {
            let first = days.first();
            let last = days.last();
            tracing::info!(
                len = days.len(),
                first = ?first,
                last = ?last,
                "start fetching data for days"
            );
            // NOTE: we update support table and chart data in one transaction
            // to support invariant that support table has information about last day in chart data
            let db_tx = db.begin().await.map_err(UpdateError::StatsDB)?;
            let data: Vec<entity::chart_data::ActiveModel> = self
                .calculate_days_using_support_table(&db_tx, blockscout, days.iter().copied())
                .await
                .map_err(|e| UpdateError::Internal(e.to_string()))?
                .into_iter()
                .map(|result| result.active_model(chart_id, Some(min_blockscout_block)))
                .collect();
            insert_data_many(&db_tx, data)
                .await
                .map_err(UpdateError::StatsDB)?;
            db_tx.commit().await.map_err(UpdateError::StatsDB)?;
        }
        Ok(())
    }

    async fn calculate_days_using_support_table<C1, C2>(
        &self,
        db: &C1,
        blockscout: &C2,
        days: impl IntoIterator<Item = NaiveDate>,
    ) -> Result<Vec<DateValue>, UpdateError>
    where
        C1: ConnectionTrait,
        C2: ConnectionTrait,
    {
        let mut result = vec![];
        let new_holders_by_date = self
            .get_holder_changes_by_date(blockscout, days)
            .await
            .map_err(|e| UpdateError::Internal(format!("cannot get new holders: {e}")))?;

        for (date, holders) in new_holders_by_date {
            // this check shouldnt be triggered if data in blockscout is correct,
            // but just in case...
            let addresses = holders.iter().map(|h| &h.address).collect::<HashSet<_>>();
            if addresses.len() != holders.len() {
                tracing::error!(addresses = ?addresses, date = ?date, "duplicate addresses in holders");
                return Err(UpdateError::Internal(
                    "duplicate addresses in holders".to_string(),
                ));
            };
            let holders = holders
                .into_iter()
                .map(|holder| db_address_balances::ActiveModel {
                    address: Set(holder.address),
                    balance: Set(holder.balance),
                });

            self.update_current_holders(db, holders)
                .await
                .map_err(|e| UpdateError::Internal(format!("cannot update holders: {e}")))?;
            let new_count = self
                .count_current_holders(db)
                .await
                .map_err(|e| UpdateError::Internal(format!("cannot count holders: {e}")))?;
            result.push(DateValue {
                date,
                value: new_count.to_string(),
            });
        }
        Ok(result)
    }

    async fn get_holder_changes_by_date<C>(
        &self,
        blockscout: &C,
        days: impl IntoIterator<Item = NaiveDate>,
    ) -> Result<BTreeMap<NaiveDate, Vec<db_address_balances::Model>>, DbErr>
    where
        C: ConnectionTrait,
    {
        let days = days.into_iter().collect::<Vec<_>>();
        let all_holders = {
            // use BTreeMap to prevent address duplicates due to several queries
            let mut all_rows: BTreeMap<Vec<u8>, address_coin_balances_daily::Model> =
                BTreeMap::new();
            let limit = self.max_rows_fetch_per_iteration();
            let mut offset = 0;
            loop {
                let rows = address_coin_balances_daily::Entity::find()
                    .filter(address_coin_balances_daily::Column::Day.is_in(days.clone()))
                    .order_by_asc(address_coin_balances_daily::Column::AddressHash)
                    .limit(limit)
                    .offset(offset)
                    .all(blockscout)
                    .await?;
                let n = rows.len() as u64;

                all_rows.extend(&mut rows.into_iter().map(|row| (row.address_hash.clone(), row)));
                if n < limit {
                    break;
                }
                offset += n;
            }
            all_rows
        };
        let holders_grouped: BTreeMap<NaiveDate, Vec<db_address_balances::Model>> = all_holders
            .into_values()
            .map(|row| {
                (
                    row.day,
                    db_address_balances::Model {
                        address: row.address_hash,
                        balance: row.value.unwrap_or_default(),
                    },
                )
            })
            .into_group_map()
            .into_iter()
            .collect();

        tracing::debug!(result =? holders_grouped, "result of get holders in days");
        Ok(holders_grouped)
    }

    async fn create_support_table(&self, db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
        let statement = Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!(
                r#"
                CREATE TABLE IF NOT EXISTS {} (
                    address BYTEA PRIMARY KEY,
                    balance NUMERIC(100,0) NOT NULL
                )
                "#,
                self.support_table_name()
            ),
        );
        db.execute(statement).await?;
        Ok(())
    }

    async fn clear_support_table(&self, db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
        let statement = Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!("DELETE FROM {}", self.support_table_name()),
        );
        db.execute(statement).await?;
        Ok(())
    }

    async fn count_current_holders<C>(&self, db: &C) -> Result<u64, DbErr>
    where
        C: ConnectionTrait,
    {
        let count = db_address_balances::Entity::find()
            .filter(db_address_balances::Column::Balance.gte(self.min_balance_for_holders()))
            .count(db)
            .await?;
        Ok(count)
    }

    async fn update_current_holders<C>(
        &self,
        db: &C,
        holders: impl IntoIterator<Item = db_address_balances::ActiveModel>,
    ) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        let mut data = holders.into_iter().peekable();
        let take = self.max_rows_insert_per_iteration();
        while data.peek().is_some() {
            let chunk: Vec<_> = data.by_ref().take(take).collect();
            db_address_balances::Entity::insert_many(chunk)
                .on_conflict(
                    OnConflict::column(db_address_balances::Column::Address)
                        .update_column(db_address_balances::Column::Balance)
                        .to_owned(),
                )
                .exec(db)
                .await?;
        }
        Ok(())
    }
}

impl NativeCoinHoldersGrowth {
    fn support_table_name(&self) -> String {
        db_address_balances::Entity.table_name().to_string()
    }

    fn min_balance_for_holders(&self) -> i64 {
        10_i64.pow(15)
    }

    fn step_duration_days(&self) -> usize {
        1
    }

    fn max_rows_fetch_per_iteration(&self) -> u64 {
        60_000
    }

    fn max_rows_insert_per_iteration(&self) -> usize {
        20_000
    }
}

async fn get_unique_ordered_days<C>(
    blockscout: &C,
    maybe_from: Option<NaiveDate>,
) -> Result<Vec<NaiveDate>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    #[derive(Debug, FromQueryResult)]
    struct SelectResult {
        day: NaiveDate,
    }

    let mut query = blockscout_db::entity::address_coin_balances_daily::Entity::find()
        .select_only()
        .column(blockscout_db::entity::address_coin_balances_daily::Column::Day)
        .group_by(blockscout_db::entity::address_coin_balances_daily::Column::Day)
        .order_by_asc(blockscout_db::entity::address_coin_balances_daily::Column::Day);

    query = match maybe_from {
        Some(from) => {
            query.filter(blockscout_db::entity::address_coin_balances_daily::Column::Day.gte(from))
        }
        None => query,
    };
    let days = query
        .into_model::<SelectResult>()
        .all(blockscout)
        .await?
        .into_iter()
        .map(|result| result.day)
        .collect();

    Ok(days)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coin_holders_growth() {
        let chart = NativeCoinHoldersGrowth::default();

        simple_test_chart(
            "update_native_coin_holders_growth",
            chart,
            vec![
                ("2022-11-08", "0"),
                ("2022-11-09", "8"),
                ("2022-11-10", "8"),
                ("2022-11-11", "7"),
            ],
        )
        .await;
    }
}
