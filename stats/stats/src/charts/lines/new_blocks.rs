use crate::{
    charts::{insert::DateValue, updater::ChartPartialUpdater},
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
        last_row: Option<DateValue>,
    ) -> Result<Vec<DateValue>, UpdateError> {
        let stmnt = match last_row {
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
    use super::*;
    use crate::{
        charts::updater::get_min_block_blockscout,
        get_chart_data,
        tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
        Chart,
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
        fill_mock_blockscout_data(&blockscout, "2022-11-12").await;

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
        updater.update(&db, &blockscout, false).await.unwrap();
        let data = get_chart_data(&db, updater.name(), None, None, None)
            .await
            .unwrap();
        let expected = vec![
            DateValue {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "3".into(),
            },
            DateValue {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "4".into(),
            },
            DateValue {
                date: NaiveDate::from_str("2022-11-12").unwrap(),
                value: "1".into(),
            },
        ];
        assert_eq!(expected, data);

        // note that update is full, therefore there is entry with date `2022-11-09`
        updater.update(&db, &blockscout, true).await.unwrap();
        let data = get_chart_data(&db, updater.name(), None, None, None)
            .await
            .unwrap();
        let expected = vec![
            DateValue {
                date: NaiveDate::from_str("2022-11-09").unwrap(),
                value: "1".into(),
            },
            DateValue {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "3".into(),
            },
            DateValue {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "4".into(),
            },
            DateValue {
                date: NaiveDate::from_str("2022-11-12").unwrap(),
                value: "1".into(),
            },
        ];
        assert_eq!(expected, data);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_blocks_fresh() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_fresh").await;
        fill_mock_blockscout_data(&blockscout, "2022-11-12").await;

        let updater = NewBlocks::default();
        updater.create(&db).await.unwrap();

        updater.update(&db, &blockscout, true).await.unwrap();
        let data = get_chart_data(&db, updater.name(), None, None, None)
            .await
            .unwrap();
        let expected = vec![
            DateValue {
                date: NaiveDate::from_str("2022-11-09").unwrap(),
                value: "1".into(),
            },
            DateValue {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "3".into(),
            },
            DateValue {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "4".into(),
            },
            DateValue {
                date: NaiveDate::from_str("2022-11-12").unwrap(),
                value: "1".into(),
            },
        ];
        assert_eq!(expected, data);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_blocks_last() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_last").await;
        fill_mock_blockscout_data(&blockscout, "2022-11-12").await;

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

        updater.update(&db, &blockscout, false).await.unwrap();
        let data = get_chart_data(&db, updater.name(), None, None, None)
            .await
            .unwrap();
        let expected = vec![
            DateValue {
                date: NaiveDate::from_str("2022-11-09").unwrap(),
                value: "2".into(),
            },
            DateValue {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "4".into(),
            },
            DateValue {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "5".into(),
            },
            DateValue {
                date: NaiveDate::from_str("2022-11-12").unwrap(),
                value: "1".into(),
            },
        ];
        assert_eq!(expected, data);
    }
}
