use crate::{
    charts::db_interaction::{
        chart_updaters::{ChartPartialUpdater, ChartUpdater},
        types::DateValue,
    },
    UpdateError,
};
use async_trait::async_trait;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, FromQueryResult, Statement};

#[derive(Default, Debug)]
pub struct NewBlocks {}

#[async_trait]
impl ChartPartialUpdater for NewBlocks {
    async fn get_values(
        &self,
        blockscout: &DatabaseConnection,
        last_updated_row: Option<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let stmnt = match last_updated_row {
            Some(row) => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                    SELECT date(blocks.timestamp) as date, COUNT(*)::TEXT as value
                        FROM public.blocks
                        WHERE 
                            blocks.timestamp != to_timestamp(0) AND
                            date(blocks.timestamp) > $1 AND
                            consensus = true
                        GROUP BY date;
                    "#,
                vec![row.date.into()],
            ),
            None => Statement::from_string(
                DbBackend::Postgres,
                r#"
                    SELECT date(blocks.timestamp) as date, COUNT(*)::TEXT as value
                        FROM public.blocks
                        WHERE 
                            blocks.timestamp != to_timestamp(0) AND 
                            consensus = true
                        GROUP BY date;
                    "#
                .into(),
            ),
        };
        let data = DateValue::find_by_statement(stmnt)
            .all(blockscout)
            .await
            .map_err(UpdateError::BlockscoutDB)?;
        Ok(data)
    }
}

#[async_trait]
impl crate::Chart for NewBlocks {
    fn name(&self) -> &str {
        "newBlocks"
    }

    fn chart_type(&self) -> ChartType {
        ChartType::Line
    }
}

#[async_trait]
impl ChartUpdater for NewBlocks {
    async fn update_values(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
        current_time: chrono::DateTime<chrono::Utc>,
        force_full: bool,
    ) -> Result<(), UpdateError> {
        self.update_with_values(db, blockscout, current_time, force_full)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        charts::db_interaction::chart_updaters::common_operations::get_min_block_blockscout,
        get_chart_data,
        tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
        Chart, ExtendedDateValue,
    };
    use chrono::NaiveDate;
    use entity::chart_data;
    use pretty_assertions::assert_eq;
    use sea_orm::Set;
    use std::str::FromStr;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_blocks_recurrent() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_recurrent").await;
        let current_time = chrono::DateTime::from_str("2022-11-12T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();
        fill_mock_blockscout_data(&blockscout, current_date).await;

        let updater = NewBlocks::default();
        updater.create(&db).await.unwrap();

        let min_blockscout_block = get_min_block_blockscout(&blockscout).await.unwrap();
        // set wrong value and check, that it was rewritten
        chart_data::Entity::insert_many([
            chart_data::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
                value: Set(3.to_string()),
                min_blockscout_block: Set(Some(min_blockscout_block)),
                ..Default::default()
            },
            chart_data::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-11").unwrap()),
                value: Set(100.to_string()),
                min_blockscout_block: Set(Some(min_blockscout_block)),
                ..Default::default()
            },
        ])
        .exec(&db as &DatabaseConnection)
        .await
        .unwrap();

        // Note that update is not full, therefore there is no entry with date `2022-11-09`
        updater
            .update(&db, &blockscout, current_time, false)
            .await
            .unwrap();
        let data = get_chart_data(&db, updater.name(), None, None, None, None, 1)
            .await
            .unwrap();
        let expected = vec![
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "3".into(),
                is_approximate: false,
            },
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "4".into(),
                is_approximate: false,
            },
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-12").unwrap(),
                value: "1".into(),
                is_approximate: true,
            },
        ];
        assert_eq!(expected, data);

        // note that update is full, therefore there is entry with date `2022-11-09`
        updater
            .update(&db, &blockscout, current_time, true)
            .await
            .unwrap();
        let data = get_chart_data(&db, updater.name(), None, None, None, None, 1)
            .await
            .unwrap();
        let expected = vec![
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-09").unwrap(),
                value: "1".into(),
                is_approximate: false,
            },
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "3".into(),
                is_approximate: false,
            },
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "4".into(),
                is_approximate: false,
            },
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-12").unwrap(),
                value: "1".into(),
                is_approximate: true,
            },
        ];
        assert_eq!(expected, data);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_blocks_fresh() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_fresh").await;
        let current_time = chrono::DateTime::from_str("2022-11-12T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();
        fill_mock_blockscout_data(&blockscout, current_date).await;

        let updater = NewBlocks::default();
        updater.create(&db).await.unwrap();

        updater
            .update(&db, &blockscout, current_time, true)
            .await
            .unwrap();
        let data = get_chart_data(&db, updater.name(), None, None, None, None, 0)
            .await
            .unwrap();
        let expected = vec![
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-09").unwrap(),
                value: "1".into(),
                is_approximate: false,
            },
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "3".into(),
                is_approximate: false,
            },
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "4".into(),
                is_approximate: false,
            },
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-12").unwrap(),
                value: "1".into(),
                is_approximate: false,
            },
        ];
        assert_eq!(expected, data);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_blocks_last() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_last").await;
        let current_time = chrono::DateTime::from_str("2022-11-12T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();
        fill_mock_blockscout_data(&blockscout, current_date).await;

        let updater = NewBlocks::default();
        updater.create(&db).await.unwrap();

        let min_blockscout_block = get_min_block_blockscout(&blockscout).await.unwrap();
        // set wrong values and check, that they wasn't rewritten
        // except the last one
        chart_data::Entity::insert_many([
            chart_data::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-09").unwrap()),
                value: Set(2.to_string()),
                min_blockscout_block: Set(Some(min_blockscout_block)),
                ..Default::default()
            },
            chart_data::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
                value: Set(4.to_string()),
                min_blockscout_block: Set(Some(min_blockscout_block)),
                ..Default::default()
            },
            chart_data::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-11").unwrap()),
                value: Set(5.to_string()),
                min_blockscout_block: Set(Some(min_blockscout_block)),
                ..Default::default()
            },
            chart_data::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-12").unwrap()),
                value: Set(2.to_string()),
                min_blockscout_block: Set(Some(min_blockscout_block)),
                ..Default::default()
            },
        ])
        .exec(&db as &DatabaseConnection)
        .await
        .unwrap();

        updater
            .update(&db, &blockscout, current_time, false)
            .await
            .unwrap();
        let data = get_chart_data(&db, updater.name(), None, None, None, None, 1)
            .await
            .unwrap();
        let expected = vec![
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-09").unwrap(),
                value: "2".into(),
                is_approximate: false,
            },
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "4".into(),
                is_approximate: false,
            },
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "5".into(),
                is_approximate: false,
            },
            ExtendedDateValue {
                date: NaiveDate::from_str("2022-11-12").unwrap(),
                value: "1".into(),
                is_approximate: true,
            },
        ];
        assert_eq!(expected, data);
    }
}
