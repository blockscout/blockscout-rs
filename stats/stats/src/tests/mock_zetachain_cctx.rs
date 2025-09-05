use std::str::FromStr;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use sea_orm::{
    ActiveValue::Set, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait, Schema,
};
use zetachain_cctx_entity::{
    cctx_status, cross_chain_tx,
    sea_orm_active_enums::{CctxStatusStatus, Kind, ProcessingStatus, ProtocolContractVersion},
    watermark,
};

/// Initialize in-memory DB with watermark for easy testing
pub async fn init_imdb_with_watermark(timestamp: Option<DateTime<Utc>>) -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(DbBackend::Sqlite);
    db.execute(
        db.get_database_backend()
            .build(&schema.create_table_from_entity(watermark::Entity)),
    )
    .await
    .unwrap();
    if let Some(t) = timestamp {
        fill_watermark(&db, t).await;
    }
    db
}

pub async fn fill_watermark(db: &DatabaseConnection, timestamp: DateTime<Utc>) {
    watermark::Entity::insert(watermark::ActiveModel {
        id: Set(1),
        kind: Set(Kind::Historical),
        upper_bound_timestamp: Set(Some(timestamp.naive_utc())),
        pointer: Set("".to_string()),
        processing_status: Set(zetachain_cctx_entity::sea_orm_active_enums::ProcessingStatus::Done),
        created_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(chrono::Utc::now().naive_utc()),
        updated_by: Set("".to_string()),
        retries_number: Set(0),
    })
    .exec(db)
    .await
    .unwrap();
}

pub async fn fill_mock_zetachain_cctx_data(
    zetachain_cctx: &DatabaseConnection,
    max_date: NaiveDate,
    set_watermark: bool,
) {
    let (cctxs, statuses): (Vec<_>, Vec<_>) = mock_cctxs(max_date).into_iter().unzip();
    cross_chain_tx::Entity::insert_many(cctxs.clone())
        .exec(zetachain_cctx)
        .await
        .unwrap();
    cctx_status::Entity::insert_many(statuses.clone())
        .exec(zetachain_cctx)
        .await
        .unwrap();
    if set_watermark {
        fill_watermark(zetachain_cctx, Utc::now()).await;
    }
}

pub async fn insert_cross_chain_txns_with_status(
    zetachain_cctx: &DatabaseConnection,
    generator: impl IntoIterator<Item = (usize, NaiveDateTime)>,
) {
    let (cctxs, statuses): (Vec<_>, Vec<_>) = generator
        .into_iter()
        .map(|(i, ts)| mock_cross_chain_tx_with_status(i, ts))
        .unzip();
    cross_chain_tx::Entity::insert_many(cctxs.clone())
        .exec(zetachain_cctx)
        .await
        .unwrap();
    cctx_status::Entity::insert_many(statuses.clone())
        .exec(zetachain_cctx)
        .await
        .unwrap();
}

fn mock_cctxs(max_date: NaiveDate) -> Vec<(cross_chain_tx::ActiveModel, cctx_status::ActiveModel)> {
    vec![
        "2022-11-09T23:59:59",
        "2022-11-10T00:00:00",
        "2022-11-10T12:00:00",
    ]
    .into_iter()
    .map(|val| NaiveDateTime::from_str(val).unwrap())
    .filter(|ts| ts.date() <= max_date)
    .enumerate()
    .map(|(i, ts)| mock_cross_chain_tx_with_status(i, ts))
    .collect()
}

fn mock_cross_chain_tx_with_status(
    index: usize,
    ts: NaiveDateTime,
) -> (cross_chain_tx::ActiveModel, cctx_status::ActiveModel) {
    (mock_cross_chain_tx(index, ts), mock_cctx_status(index, ts))
}

fn mock_cross_chain_tx(index: usize, ts: NaiveDateTime) -> cross_chain_tx::ActiveModel {
    cross_chain_tx::ActiveModel {
        id: Set(index as i32),
        creator: Set("me".to_string()),
        index: Set(format!("{index}")),
        zeta_fees: Set("0".to_string()),
        retries_number: Set(0),
        processing_status: Set(ProcessingStatus::Done),
        relayed_message: Set(None),
        last_status_update_timestamp: Set(ts),
        protocol_contract_version: Set(ProtocolContractVersion::V1),
        root_id: Set(None),
        parent_id: Set(None),
        depth: Set(0),
        updated_by: Set("me".to_string()),
        token_id: Set(None),
        receiver_chain_id: Set(0),
        receiver: Set("me".to_string()),
    }
}

fn mock_cctx_status(index: usize, ts: NaiveDateTime) -> cctx_status::ActiveModel {
    let status = if index % 2 == 0 {
        CctxStatusStatus::OutboundMined
    } else {
        CctxStatusStatus::PendingRevert
    };
    cctx_status::ActiveModel {
        id: Set(index as i32),
        cross_chain_tx_id: Set(index as i32),
        status: Set(status),
        status_message: Set(None),
        error_message: Set(None),
        last_update_timestamp: Set(ts),
        is_abort_refunded: Set(false),
        created_timestamp: Set(ts.and_utc().timestamp()),
        error_message_revert: Set(None),
        error_message_abort: Set(None),
    }
}
