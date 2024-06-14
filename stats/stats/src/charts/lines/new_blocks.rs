use std::ops::RangeInclusive;

use crate::{
    data_source::kinds::{
        local_db::DirectVecLocalDbChartSource,
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
    },
    utils::sql_with_range_filter_opt,
    ChartProperties, DateValueString, Named,
};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

pub struct NewBlocksStatement;

impl StatementFromRange for NewBlocksStatement {
    fn get_statement(range: Option<RangeInclusive<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                    SELECT
                        date(blocks.timestamp) as date,
                        COUNT(*)::TEXT as value
                    FROM public.blocks
                    WHERE 
                        blocks.timestamp != to_timestamp(0) AND
                        consensus = true {filter}
                    GROUP BY date;
                "#,
            [],
            "blocks.timestamp",
            range
        )
    }
}

pub type NewBlocksRemote =
    RemoteDatabaseSource<PullAllWithAndSort<NewBlocksStatement, DateValueString>>;

pub struct NewBlocksProperties;

impl Named for NewBlocksProperties {
    const NAME: &'static str = "newBlocks";
}

impl ChartProperties for NewBlocksProperties {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type NewBlocks = DirectVecLocalDbChartSource<NewBlocksRemote, NewBlocksProperties>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        charts::db_interaction::read::get_min_block_blockscout,
        data_source::{DataSource, UpdateContext},
        get_line_chart_data,
        tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
        ExtendedDateValue, Named,
    };
    use chrono::{NaiveDate, Utc};
    use entity::chart_data;
    use pretty_assertions::assert_eq;
    use sea_orm::{DatabaseConnection, EntityTrait, Set};
    use std::str::FromStr;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_blocks_recurrent() {
        let _ = tracing_subscriber::fmt::try_init();
        let (db, blockscout) = init_db_all("update_new_blocks_recurrent").await;
        let current_time = chrono::DateTime::<Utc>::from_str("2022-11-12T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();
        fill_mock_blockscout_data(&blockscout, current_date).await;

        NewBlocks::init_recursively(&db, &current_time)
            .await
            .unwrap();

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
        let mut cx = UpdateContext {
            db: &db,
            blockscout: &blockscout,
            time: current_time,
            force_full: false,
        };
        NewBlocks::update_recursively(&cx).await.unwrap();
        let data = get_line_chart_data(
            &db,
            NewBlocks::NAME,
            None,
            None,
            None,
            crate::MissingDatePolicy::FillZero,
            false,
            1,
        )
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
        cx.force_full = true;
        // need to update time so that the update is not ignored as the same one
        cx.time = chrono::DateTime::<Utc>::from_str("2022-11-12T13:00:00Z").unwrap();
        NewBlocks::update_recursively(&cx).await.unwrap();
        let data = get_line_chart_data(
            &db,
            NewBlocks::NAME,
            None,
            None,
            None,
            crate::MissingDatePolicy::FillZero,
            false,
            1,
        )
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

        NewBlocks::init_recursively(&db, &current_time)
            .await
            .unwrap();

        let cx = UpdateContext {
            db: &db,
            blockscout: &blockscout,
            time: current_time,
            force_full: true,
        };
        NewBlocks::update_recursively(&cx).await.unwrap();
        let data = get_line_chart_data(
            &db,
            NewBlocks::NAME,
            None,
            None,
            None,
            crate::MissingDatePolicy::FillZero,
            false,
            0,
        )
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

        NewBlocks::init_recursively(&db, &current_time)
            .await
            .unwrap();

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

        let cx = UpdateContext {
            db: &db,
            blockscout: &blockscout,
            time: current_time,
            force_full: false,
        };
        NewBlocks::update_recursively(&cx).await.unwrap();
        let data = get_line_chart_data(
            &db,
            NewBlocks::NAME,
            None,
            None,
            None,
            crate::MissingDatePolicy::FillZero,
            false,
            1,
        )
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
