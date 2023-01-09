use crate::{
    charts::insert::{insert_int_data_many, IntValueItem},
    UpdateError,
};
use async_trait::async_trait;
use chrono::NaiveDate;
use entity::{
    chart_data_int,
    sea_orm_active_enums::{ChartType, ChartValueType},
};
use sea_orm::{prelude::*, DbBackend, FromQueryResult, QueryOrder, QuerySelect, Statement};

#[derive(Debug, FromQueryResult)]
struct ChartDate {
    date: NaiveDate,
}

#[derive(Default, Debug)]
pub struct NewBlocks {}

#[async_trait]
impl crate::Chart for NewBlocks {
    fn name(&self) -> &str {
        "newBlocks"
    }

    async fn create(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        crate::charts::create_chart(db, self.name().into(), ChartType::Line, ChartValueType::Int)
            .await
    }

    async fn update(
        &self,
        db: &DatabaseConnection,
        blockscout: &DatabaseConnection,
    ) -> Result<(), UpdateError> {
        let id = crate::charts::find_chart(db, self.name())
            .await?
            .ok_or_else(|| UpdateError::NotFound(self.name().into()))?;
        let last_row = chart_data_int::Entity::find()
            .column(chart_data_int::Column::Date)
            .filter(chart_data_int::Column::ChartId.eq(id))
            .order_by_desc(chart_data_int::Column::Date)
            .into_model::<ChartDate>()
            .one(db)
            .await?;

        // TODO: rewrite using orm/build request with `where` clause
        let data = match last_row {
            Some(row) => {
                IntValueItem::find_by_statement(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    r#"
                    SELECT date(blocks.timestamp) as date, COUNT(*) as value
                        FROM public.blocks
                        WHERE date(blocks.timestamp) >= $1
                        GROUP BY date;
                    "#,
                    vec![row.date.into()],
                ))
                .all(blockscout)
                .await?
            }
            None => {
                IntValueItem::find_by_statement(Statement::from_string(
                    DbBackend::Postgres,
                    r#"
                    SELECT date(blocks.timestamp) as date, COUNT(*) as value
                        FROM public.blocks
                        GROUP BY date;
                    "#
                    .into(),
                ))
                .all(blockscout)
                .await?
            }
        };

        let data = data.into_iter().map(|item| item.active_model(id));
        insert_int_data_many(db, data).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_chart_data, tests::init_db::init_db_all, Chart};
    use blockscout_db::entity::{addresses, blocks};
    use chrono::NaiveDateTime;
    use pretty_assertions::assert_eq;
    use sea_orm::Set;
    use stats_proto::blockscout::stats::v1::{LineChart, Point};
    use std::str::FromStr;

    fn mock_block(index: i64, ts: &str) -> blocks::ActiveModel {
        blocks::ActiveModel {
            number: Set(index),
            hash: Set(index.to_le_bytes().to_vec()),
            timestamp: Set(NaiveDateTime::from_str(ts).unwrap()),
            consensus: Set(Default::default()),
            gas_limit: Set(Default::default()),
            gas_used: Set(Default::default()),
            miner_hash: Set(Default::default()),
            nonce: Set(Default::default()),
            parent_hash: Set(Default::default()),
            inserted_at: Set(Default::default()),
            updated_at: Set(Default::default()),
            ..Default::default()
        }
    }

    async fn mock_blockscout(blockscout: &DatabaseConnection) {
        addresses::Entity::insert(addresses::ActiveModel {
            hash: Set(vec![]),
            inserted_at: Set(Default::default()),
            updated_at: Set(Default::default()),
            ..Default::default()
        })
        .exec(blockscout)
        .await
        .unwrap();

        let block_timestamps = vec![
            "2022-11-09T23:59:59",
            "2022-11-10T00:00:00",
            "2022-11-10T12:00:00",
            "2022-11-10T23:59:59",
            "2022-11-11T00:00:00",
            "2022-11-11T12:00:00",
            "2022-11-11T15:00:00",
            "2022-11-11T23:59:59",
            "2022-11-12T00:00:00",
        ];
        blocks::Entity::insert_many(
            block_timestamps
                .into_iter()
                .enumerate()
                .map(|(ind, ts)| mock_block(ind as i64, ts)),
        )
        .exec(blockscout)
        .await
        .unwrap();
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_blocks_recurrent() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_recurrent", None).await;

        let updater = NewBlocks::default();
        updater.create(&db).await.unwrap();

        // set wrong value and check, that it was rewritten
        chart_data_int::Entity::insert(chart_data_int::ActiveModel {
            chart_id: Set(1),
            date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
            value: Set(100),
            ..Default::default()
        })
        .exec(&db)
        .await
        .unwrap();

        mock_blockscout(&blockscout).await;

        updater.update(&db, &blockscout).await.unwrap();
        let data = get_chart_data(&db, updater.name(), None, None)
            .await
            .unwrap();
        let expected = LineChart {
            chart: vec![
                Point {
                    date: "2022-11-10".into(),
                    value: "3".into(),
                },
                Point {
                    date: "2022-11-11".into(),
                    value: "4".into(),
                },
                Point {
                    date: "2022-11-12".into(),
                    value: "1".into(),
                },
            ],
        };
        assert_eq!(expected, data);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_blocks_fresh() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_fresh", None).await;

        let updater = NewBlocks::default();
        updater.create(&db).await.unwrap();

        mock_blockscout(&blockscout).await;

        updater.update(&db, &blockscout).await.unwrap();
        let data = get_chart_data(&db, updater.name(), None, None)
            .await
            .unwrap();
        let expected = LineChart {
            chart: vec![
                Point {
                    date: "2022-11-09".into(),
                    value: "1".into(),
                },
                Point {
                    date: "2022-11-10".into(),
                    value: "3".into(),
                },
                Point {
                    date: "2022-11-11".into(),
                    value: "4".into(),
                },
                Point {
                    date: "2022-11-12".into(),
                    value: "1".into(),
                },
            ],
        };
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
        chart_data_int::Entity::insert_many([
            chart_data_int::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-09").unwrap()),
                value: Set(2),
                ..Default::default()
            },
            chart_data_int::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-10").unwrap()),
                value: Set(4),
                ..Default::default()
            },
            chart_data_int::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-11").unwrap()),
                value: Set(5),
                ..Default::default()
            },
            chart_data_int::ActiveModel {
                chart_id: Set(1),
                date: Set(NaiveDate::from_str("2022-11-12").unwrap()),
                value: Set(2),
                ..Default::default()
            },
        ])
        .exec(&db)
        .await
        .unwrap();

        mock_blockscout(&blockscout).await;

        updater.update(&db, &blockscout).await.unwrap();
        let data = get_chart_data(&db, updater.name(), None, None)
            .await
            .unwrap();
        let expected = LineChart {
            chart: vec![
                Point {
                    date: "2022-11-09".into(),
                    value: "2".into(),
                },
                Point {
                    date: "2022-11-10".into(),
                    value: "4".into(),
                },
                Point {
                    date: "2022-11-11".into(),
                    value: "5".into(),
                },
                Point {
                    date: "2022-11-12".into(),
                    value: "1".into(),
                },
            ],
        };
        assert_eq!(expected, data);
    }
}
