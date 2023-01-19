use super::utils::OnlyDate;
use crate::{
    charts::insert::{insert_data_many, DateValue},
    UpdateError,
};
use async_trait::async_trait;
use entity::{chart_data, sea_orm_active_enums::ChartType};
use sea_orm::{prelude::*, DbBackend, FromQueryResult, QueryOrder, QuerySelect, Statement};

#[derive(Default, Debug)]
pub struct NewBlocks {}

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
        full: bool,
    ) -> Result<(), UpdateError> {
        let id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let last_row = if full {
            None
        } else {
            chart_data::Entity::find()
                .column(chart_data::Column::Date)
                .filter(chart_data::Column::ChartId.eq(id))
                .order_by_desc(chart_data::Column::Date)
                .into_model::<OnlyDate>()
                .one(db)
                .await?
        };

        // TODO: rewrite using orm/build request with `where` clause
        let data = match last_row {
            Some(row) => {
                DateValue::find_by_statement(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    r#"
                    SELECT date(blocks.timestamp) as date, COUNT(*)::TEXT as value
                        FROM public.blocks
                        WHERE date(blocks.timestamp) >= $1 AND consensus = true
                        GROUP BY date;
                    "#,
                    vec![row.date.into()],
                ))
                .all(blockscout)
                .await?
            }
            None => {
                DateValue::find_by_statement(Statement::from_string(
                    DbBackend::Postgres,
                    r#"
                    SELECT date(blocks.timestamp) as date, COUNT(*)::TEXT as value
                        FROM public.blocks
                        WHERE consensus = true
                        GROUP BY date;
                    "#
                    .into(),
                ))
                .all(blockscout)
                .await?
            }
        };

        let data = data.into_iter().map(|item| item.active_model(id));
        insert_data_many(db, data).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        get_chart_data,
        tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
        Chart, Point,
    };
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;
    use sea_orm::Set;
    use std::str::FromStr;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_blocks_recurrent() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_recurrent", None).await;

        let updater = NewBlocks::default();
        updater.create(&db).await.unwrap();

        // set wrong value and check, that it was rewritten
        chart_data::Entity::insert(chart_data::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
            value: Set(100.to_string()),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        fill_mock_blockscout_data(&blockscout, "2022-11-12").await;

        // Note that update is not full, therefore there is no entry with date `2022-11-09`
        updater.update(&db, &blockscout, false).await.unwrap();
        let data = get_chart_data(&db, updater.name(), None, None)
            .await
            .unwrap();
        let expected = vec![
            Point {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "3".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "4".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-12").unwrap(),
                value: "1".into(),
            },
        ];
        assert_eq!(expected, data);

        // note that update is full, therefore there is entry with date `2022-11-09`
        updater.update(&db, &blockscout, true).await.unwrap();
        let data = get_chart_data(&db, updater.name(), None, None)
            .await
            .unwrap();
        let expected = vec![
            Point {
                date: NaiveDate::from_str("2022-11-09").unwrap(),
                value: "1".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "3".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "4".into(),
            },
            Point {
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
        let (db, blockscout) = init_db_all("update_new_blocks_fresh", None).await;

        let updater = NewBlocks::default();
        updater.create(&db).await.unwrap();

        fill_mock_blockscout_data(&blockscout, "2022-11-12").await;

        updater.update(&db, &blockscout, true).await.unwrap();
        let data = get_chart_data(&db, updater.name(), None, None)
            .await
            .unwrap();
        let expected = vec![
            Point {
                date: NaiveDate::from_str("2022-11-09").unwrap(),
                value: "1".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "3".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "4".into(),
            },
            Point {
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
        let (db, blockscout) = init_db_all("update_new_blocks_last", None).await;

        let updater = NewBlocks::default();
        updater.create(&db).await.unwrap();

        // set wrong values and check, that they wasn't rewritten
        // except the last one
        chart_data::Entity::insert_many([
            chart_data::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-09").unwrap()),
                value: Set(2.to_string()),
                ..Default::default()
            },
            chart_data::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
                value: Set(4.to_string()),
                ..Default::default()
            },
            chart_data::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-11").unwrap()),
                value: Set(5.to_string()),
                ..Default::default()
            },
            chart_data::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-12").unwrap()),
                value: Set(2.to_string()),
                ..Default::default()
            },
        ])
        .exec(&db)
        .await
        .unwrap();

        fill_mock_blockscout_data(&blockscout, "2022-11-12").await;

        updater.update(&db, &blockscout, false).await.unwrap();
        let data = get_chart_data(&db, updater.name(), None, None)
            .await
            .unwrap();
        let expected = vec![
            Point {
                date: NaiveDate::from_str("2022-11-09").unwrap(),
                value: "2".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-10").unwrap(),
                value: "4".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-11").unwrap(),
                value: "5".into(),
            },
            Point {
                date: NaiveDate::from_str("2022-11-12").unwrap(),
                value: "1".into(),
            },
        ];
        assert_eq!(expected, data);
    }
}
