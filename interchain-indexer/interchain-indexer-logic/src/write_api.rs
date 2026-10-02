// SPDX-License-Identifier: LicenseRef-Blockscout

//! Database operations of the operator write API.
//!
//! Every function runs on the caller's [`DatabaseTransaction`] and never
//! commits: the caller decides when the change and its audit row become
//! visible together.

use interchain_indexer_entity::{stats_asset_tokens, stats_assets, write_api_audit_log};
use sea_orm::{
    ActiveValue, ColumnTrait, ConnectionTrait, DatabaseTransaction, DbErr, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, RuntimeErr, sea_query::LockType, sqlx,
};
use std::time::Duration;

/// PostgreSQL SQLSTATE `lock_not_available`, raised when `lock_timeout` expires.
const SQLSTATE_LOCK_NOT_AVAILABLE: &str = "55P03";

/// Inserts one audit row inside the caller's transaction and returns its id.
///
/// `id` and `occurred_at` are left unset so the database defaults apply.
pub async fn insert_audit_entry(
    tx: &DatabaseTransaction,
    actor: &str,
    method: &str,
    reason: &str,
    request: serde_json::Value,
    result: serde_json::Value,
) -> Result<i64, DbErr> {
    let row = write_api_audit_log::Entity::insert(write_api_audit_log::ActiveModel {
        actor: ActiveValue::Set(actor.to_owned()),
        method: ActiveValue::Set(method.to_owned()),
        reason: ActiveValue::Set(reason.to_owned()),
        request: ActiveValue::Set(request),
        result: ActiveValue::Set(result),
        ..Default::default()
    })
    .exec_with_returning(tx)
    .await?;
    Ok(row.id)
}

/// True for PostgreSQL SQLSTATE `55P03` (`lock_not_available`), raised by `lock_timeout`.
pub fn is_lock_not_available(err: &DbErr) -> bool {
    match err {
        DbErr::Exec(RuntimeErr::SqlxError(sqlx::Error::Database(db_err)))
        | DbErr::Query(RuntimeErr::SqlxError(sqlx::Error::Database(db_err))) => {
            db_err.code().as_deref() == Some(SQLSTATE_LOCK_NOT_AVAILABLE)
        }
        _ => false,
    }
}

/// State of a `stats_assets` row around an icon change, with the tokens that
/// make up the asset.
pub struct StatsAssetIconChange {
    pub before: stats_assets::Model,
    pub after: stats_assets::Model,
    pub member_tokens: Vec<stats_asset_tokens::Model>,
}

/// Sets or clears (`icon_url = None`) `stats_assets.icon_url` for one asset.
///
/// Returns `Ok(None)` when no asset has this id (for example, it lost a merge).
/// Only `icon_url` and `updated_at` are written; `tokens` is never touched.
///
/// The row is locked `FOR NO KEY UPDATE`, not `FOR UPDATE`: stats maintenance
/// takes `FOR KEY SHARE` on it through foreign-key checks, which `NO KEY
/// UPDATE` does not conflict with. `lock_timeout` bounds the wait for a
/// concurrent writer; its expiry surfaces as an error for which
/// [`is_lock_not_available`] is true.
pub async fn set_stats_asset_icon_tx(
    tx: &DatabaseTransaction,
    stats_asset_id: i64,
    icon_url: Option<&str>,
    lock_timeout: Duration,
) -> Result<Option<StatsAssetIconChange>, DbErr> {
    // The value is an integer derived from a `Duration`, not user input. `SET
    // LOCAL` lasts until the end of this transaction.
    tx.execute_unprepared(&format!(
        "SET LOCAL lock_timeout = '{}ms'",
        lock_timeout.as_millis()
    ))
    .await?;

    let Some(before) = stats_assets::Entity::find_by_id(stats_asset_id)
        .lock(LockType::NoKeyUpdate)
        .one(tx)
        .await?
    else {
        return Ok(None);
    };

    let after = stats_assets::Entity::update(stats_assets::ActiveModel {
        id: ActiveValue::Unchanged(stats_asset_id),
        icon_url: ActiveValue::Set(icon_url.map(str::to_owned)),
        updated_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        ..Default::default()
    })
    .exec(tx)
    .await?;

    // Deliberately unlocked: merge locks `stats_asset_tokens` rows while it
    // already holds the asset rows, so locking them here would build a
    // deadlock cycle with it.
    let member_tokens = stats_asset_tokens::Entity::find()
        .filter(stats_asset_tokens::Column::StatsAssetId.eq(stats_asset_id))
        .order_by_asc(stats_asset_tokens::Column::ChainId)
        .all(tx)
        .await?;

    Ok(Some(StatsAssetIconChange {
        before,
        after,
        member_tokens,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{IndexedChains, InterchainDatabase, test_utils::init_db};
    use bigdecimal::BigDecimal;
    use chrono::{NaiveDate, NaiveDateTime, Utc};
    use interchain_indexer_entity::{
        bridges, chains, crosschain_messages, crosschain_transfers,
        sea_orm_active_enums::{EdgeAmountSide, MessageStatus, TokenType, TransferAssetLinkage},
        stats_asset_edges, tokens,
    };
    use pretty_assertions::assert_eq;
    use sea_orm::{DatabaseConnection, TransactionTrait};
    use serde_json::json;

    const OLD_ICON: &str = "https://old.example/i.png";
    const NEW_ICON: &str = "https://new.example/i.png";
    const TOKEN_ICON: &str = "https://token.example/t.png";

    fn old_timestamp() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2020, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    }

    async fn seed_chains_and_bridge(db: &DatabaseConnection, chain_ids: &[i64]) {
        chains::Entity::insert_many(chain_ids.iter().map(|id| chains::ActiveModel {
            id: ActiveValue::Set(*id),
            name: ActiveValue::Set(format!("chain{id}")),
            ..Default::default()
        }))
        .exec(db)
        .await
        .unwrap();
        bridges::Entity::insert(bridges::ActiveModel {
            id: ActiveValue::Set(1),
            name: ActiveValue::Set("Br".into()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();
    }

    async fn seed_asset(
        db: &DatabaseConnection,
        name: Option<&str>,
        symbol: Option<&str>,
        icon_url: Option<&str>,
    ) -> i64 {
        stats_assets::Entity::insert(stats_assets::ActiveModel {
            name: ActiveValue::Set(name.map(str::to_owned)),
            symbol: ActiveValue::Set(symbol.map(str::to_owned)),
            icon_url: ActiveValue::Set(icon_url.map(str::to_owned)),
            created_at: ActiveValue::Set(old_timestamp()),
            updated_at: ActiveValue::Set(old_timestamp()),
            ..Default::default()
        })
        .exec_with_returning(db)
        .await
        .unwrap()
        .id
    }

    async fn link_token(
        db: &DatabaseConnection,
        stats_asset_id: i64,
        chain_id: i64,
        address: Vec<u8>,
        token_type: TokenType,
    ) {
        stats_asset_tokens::Entity::insert(stats_asset_tokens::ActiveModel {
            stats_asset_id: ActiveValue::Set(stats_asset_id),
            chain_id: ActiveValue::Set(chain_id),
            token_address: ActiveValue::Set(address),
            r#type: ActiveValue::Set(token_type),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();
    }

    async fn seed_token_row(
        db: &DatabaseConnection,
        chain_id: i64,
        address: Vec<u8>,
        name: Option<&str>,
        icon: Option<&str>,
    ) -> tokens::Model {
        tokens::Entity::insert(tokens::ActiveModel {
            chain_id: ActiveValue::Set(chain_id),
            address: ActiveValue::Set(address),
            name: ActiveValue::Set(name.map(str::to_owned)),
            token_icon: ActiveValue::Set(icon.map(str::to_owned)),
            ..Default::default()
        })
        .exec_with_returning(db)
        .await
        .unwrap()
    }

    async fn asset(db: &DatabaseConnection, id: i64) -> stats_assets::Model {
        stats_assets::Entity::find_by_id(id)
            .one(db)
            .await
            .unwrap()
            .expect("asset exists")
    }

    /// Runs `set_stats_asset_icon_tx` in its own transaction and commits it.
    async fn set_icon_committed(
        db: &DatabaseConnection,
        id: i64,
        icon_url: Option<&str>,
    ) -> StatsAssetIconChange {
        let tx = db.begin().await.unwrap();
        let change = set_stats_asset_icon_tx(&tx, id, icon_url, Duration::from_secs(5))
            .await
            .unwrap()
            .expect("asset exists");
        tx.commit().await.unwrap();
        change
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn write_api_db_insert_audit_entry_returns_id_and_defaults_occurred_at() {
        let guard = init_db("write_api_db_insert_audit_entry").await;
        let conn = guard.client();
        let db = conn.as_ref();

        let tx = db.begin().await.unwrap();
        let first = insert_audit_entry(
            &tx,
            "ops_alice",
            "SetStatsAssetIcon",
            "TICKET-1",
            json!({"stats_asset_id": 7}),
            json!({"icon_url_after": NEW_ICON}),
        )
        .await
        .unwrap();
        let second = insert_audit_entry(&tx, "ops_alice", "M", "r", json!({}), json!({}))
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert!(second > first, "ids come from the BIGSERIAL sequence");

        let row = write_api_audit_log::Entity::find_by_id(first)
            .one(db)
            .await
            .unwrap()
            .expect("row was committed");
        assert_eq!(row.actor, "ops_alice");
        assert_eq!(row.method, "SetStatsAssetIcon");
        assert_eq!(row.reason, "TICKET-1");
        assert_eq!(row.request, json!({"stats_asset_id": 7}));
        assert_eq!(row.result, json!({"icon_url_after": NEW_ICON}));
        let skew = Utc::now().naive_utc() - row.occurred_at;
        assert!(
            skew.num_hours().abs() < 24,
            "occurred_at must come from the DB default now(), got {}",
            row.occurred_at
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn write_api_db_set_asset_icon_changes_only_stats_assets() {
        let guard = init_db("write_api_db_set_asset_icon_changes_only").await;
        let conn = guard.client();
        let db = conn.as_ref();
        seed_chains_and_bridge(db, &[1, 100]).await;

        let addr_erc20 = vec![0xa1u8; 20];
        let addr_native = vec![0u8; 20];
        let id = seed_asset(db, Some("USDC"), Some("USDC"), Some(OLD_ICON)).await;
        // Inserted out of chain order: members must come back ordered by chain.
        link_token(db, id, 100, addr_native.clone(), TokenType::Native).await;
        link_token(db, id, 1, addr_erc20.clone(), TokenType::Erc20).await;
        let token_before =
            seed_token_row(db, 1, addr_erc20.clone(), Some("USDC"), Some(TOKEN_ICON)).await;

        let change = set_icon_committed(db, id, Some(NEW_ICON)).await;

        assert_eq!(change.before.icon_url.as_deref(), Some(OLD_ICON));
        assert_eq!(change.after.icon_url.as_deref(), Some(NEW_ICON));
        assert_eq!(change.before.updated_at, old_timestamp());
        assert!(change.after.updated_at > old_timestamp());
        assert_eq!(
            change
                .member_tokens
                .iter()
                .map(|m| (m.chain_id, m.r#type.clone()))
                .collect::<Vec<_>>(),
            vec![(1, TokenType::Erc20), (100, TokenType::Native)]
        );

        let stored = asset(db, id).await;
        assert_eq!(stored, change.after, "RETURNING matches the stored row");
        assert_eq!(stored.name.as_deref(), Some("USDC"));
        assert_eq!(stored.symbol.as_deref(), Some("USDC"));
        assert_eq!(stored.created_at, old_timestamp());

        let token_after = tokens::Entity::find_by_id((1i64, addr_erc20))
            .one(db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(token_after, token_before, "tokens must not be touched");
        assert_eq!(token_after.token_icon.as_deref(), Some(TOKEN_ICON));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn write_api_db_set_asset_icon_unknown_id_returns_none() {
        let guard = init_db("write_api_db_set_asset_icon_unknown_id").await;
        let conn = guard.client();
        let db = conn.as_ref();
        let existing = seed_asset(db, None, None, Some(OLD_ICON)).await;

        let tx = db.begin().await.unwrap();
        let change =
            set_stats_asset_icon_tx(&tx, existing + 1000, Some(NEW_ICON), Duration::from_secs(5))
                .await
                .unwrap();
        assert!(change.is_none());
        tx.rollback().await.unwrap();

        assert_eq!(
            asset(db, existing).await.icon_url.as_deref(),
            Some(OLD_ICON)
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn write_api_db_clear_sets_null_and_propagate_refills_it() {
        let guard = init_db("write_api_db_clear_then_propagate").await;
        let conn = guard.client();
        let db = conn.as_ref();
        seed_chains_and_bridge(db, &[1, 100]).await;
        let addr = vec![0xb1u8; 20];
        let id = seed_asset(db, None, None, Some(OLD_ICON)).await;
        link_token(db, id, 100, addr.clone(), TokenType::Erc20).await;
        let token = seed_token_row(db, 100, addr.clone(), Some("Tok"), Some(TOKEN_ICON)).await;

        let change = set_icon_committed(db, id, None).await;
        assert_eq!(change.before.icon_url.as_deref(), Some(OLD_ICON));
        assert_eq!(change.after.icon_url, None);
        assert_eq!(asset(db, id).await.icon_url, None, "clear stores NULL");

        InterchainDatabase::new(conn.clone())
            .propagate_token_info_to_stats_tables(100, &addr, &token)
            .await
            .unwrap();
        assert_eq!(
            asset(db, id).await.icon_url.as_deref(),
            Some(TOKEN_ICON),
            "an empty asset icon is refilled from the member token"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn write_api_db_propagate_does_not_replace_a_manual_asset_icon() {
        let guard = init_db("write_api_db_propagate_keeps_manual_icon").await;
        let conn = guard.client();
        let db = conn.as_ref();
        seed_chains_and_bridge(db, &[1, 100]).await;
        let addr = vec![0xb2u8; 20];
        let id = seed_asset(db, None, None, None).await;
        link_token(db, id, 100, addr.clone(), TokenType::Erc20).await;
        let token = seed_token_row(db, 100, addr.clone(), Some("Tok"), Some(TOKEN_ICON)).await;

        set_icon_committed(db, id, Some(NEW_ICON)).await;

        InterchainDatabase::new(conn.clone())
            .propagate_token_info_to_stats_tables(100, &addr, &token)
            .await
            .unwrap();
        let stored = asset(db, id).await;
        assert_eq!(stored.icon_url.as_deref(), Some(NEW_ICON));
        assert_eq!(
            stored.name.as_deref(),
            Some("Tok"),
            "propagate did run: it filled the empty name but kept the manual icon"
        );
    }

    fn completed_message(id: i64, src: i64, dst: i64) -> crosschain_messages::ActiveModel {
        crosschain_messages::ActiveModel {
            id: ActiveValue::Set(id),
            bridge_id: ActiveValue::Set(1),
            status: ActiveValue::Set(MessageStatus::Completed),
            init_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
            src_chain_id: ActiveValue::Set(src),
            dst_chain_id: ActiveValue::Set(Some(dst)),
            src_tx_hash: ActiveValue::Set(Some(vec![0xabu8; 32])),
            stats_processed: ActiveValue::Set(0),
            ..Default::default()
        }
    }

    /// Inserts a mirror transfer between two token endpoints and projects it
    /// into the stats tables in one transaction, as stats maintenance does.
    async fn project_mirror_transfer(
        db: &DatabaseConnection,
        id: i64,
        src: (i64, Vec<u8>),
        dst: (i64, Vec<u8>),
    ) {
        crosschain_messages::Entity::insert(completed_message(id, src.0, dst.0))
            .exec(db)
            .await
            .unwrap();
        crosschain_transfers::Entity::insert(crosschain_transfers::ActiveModel {
            id: ActiveValue::Set(id),
            message_id: ActiveValue::Set(id),
            bridge_id: ActiveValue::Set(1),
            index: ActiveValue::Set(0),
            token_src_chain_id: ActiveValue::Set(src.0),
            token_dst_chain_id: ActiveValue::Set(dst.0),
            src_amount: ActiveValue::Set(Some(BigDecimal::from(1u64))),
            dst_amount: ActiveValue::Set(Some(BigDecimal::from(1u64))),
            token_src_address: ActiveValue::Set(Some(src.1)),
            token_dst_address: ActiveValue::Set(Some(dst.1)),
            asset_linkage: ActiveValue::Set(Some(TransferAssetLinkage::Mirror)),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();

        db.transaction(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(
                    tx,
                    &[(id, 1i32)],
                    &IndexedChains::AllIndexed,
                )
                .await?;
                crate::stats::projection::project_transfers_batch(
                    tx,
                    &[id],
                    &IndexedChains::AllIndexed,
                )
                .await?;
                Ok::<(), DbErr>(())
            })
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn write_api_db_projection_does_not_replace_a_manual_asset_icon() {
        let guard = init_db("write_api_db_projection_keeps_manual_icon").await;
        let conn = guard.client();
        let db = conn.as_ref();
        seed_chains_and_bridge(db, &[1, 100]).await;
        let addr_src = vec![0xc1u8; 20];
        let addr_dst = vec![0xc2u8; 20];
        let id = seed_asset(db, None, None, None).await;
        link_token(db, id, 1, addr_src.clone(), TokenType::Erc20).await;
        link_token(db, id, 100, addr_dst.clone(), TokenType::Erc20).await;
        seed_token_row(db, 100, addr_dst.clone(), Some("Tok"), Some(TOKEN_ICON)).await;

        set_icon_committed(db, id, Some(NEW_ICON)).await;
        project_mirror_transfer(db, 93001, (1, addr_src), (100, addr_dst)).await;

        let stored = asset(db, id).await;
        assert_eq!(stored.icon_url.as_deref(), Some(NEW_ICON));
        assert_eq!(
            stored.name.as_deref(),
            Some("Tok"),
            "projection did run: it filled the empty name but kept the manual icon"
        );
    }

    /// Seeds a one-token loser and a three-token winner on four fresh chains,
    /// merges them by projecting a mirror transfer between the two, and returns
    /// the surviving winner row (asserting the loser is gone).
    async fn merge_with_icons(
        db: &DatabaseConnection,
        first_chain: i64,
        winner_icon: Option<&str>,
        loser_icon: Option<&str>,
    ) -> stats_assets::Model {
        chains::Entity::insert_many(
            (first_chain..first_chain + 4).map(|id| chains::ActiveModel {
                id: ActiveValue::Set(id),
                name: ActiveValue::Set(format!("chain{id}")),
                ..Default::default()
            }),
        )
        .exec(db)
        .await
        .unwrap();
        // The loser is created first (lower id) so that size, not id, decides.
        let loser = seed_asset(db, None, None, loser_icon).await;
        let winner = seed_asset(db, None, None, winner_icon).await;
        let loser_addr = vec![0x01u8; 20];
        link_token(db, loser, first_chain, loser_addr.clone(), TokenType::Erc20).await;
        let winner_addr = vec![0x02u8; 20];
        for offset in 1..=3 {
            link_token(
                db,
                winner,
                first_chain + offset,
                winner_addr.clone(),
                TokenType::Erc20,
            )
            .await;
        }

        project_mirror_transfer(
            db,
            93000 + first_chain,
            (first_chain, loser_addr),
            (first_chain + 1, winner_addr),
        )
        .await;

        assert!(
            stats_assets::Entity::find_by_id(loser)
                .one(db)
                .await
                .unwrap()
                .is_none(),
            "the smaller component loses the merge and is deleted"
        );
        asset(db, winner).await
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn write_api_db_merge_icon_precedence_is_pinned() {
        let guard = init_db("write_api_db_merge_icon_precedence").await;
        let conn = guard.client();
        let db = conn.as_ref();
        seed_chains_and_bridge(db, &[1, 100]).await;

        // The winner's icon is kept.
        let winner = merge_with_icons(db, 510, Some("https://winner.example/w.png"), None).await;
        assert_eq!(
            winner.icon_url.as_deref(),
            Some("https://winner.example/w.png")
        );

        // An empty winner takes the loser's icon.
        let winner = merge_with_icons(db, 520, None, Some("https://loser.example/l.png")).await;
        assert_eq!(
            winner.icon_url.as_deref(),
            Some("https://loser.example/l.png")
        );

        // A non-empty winner keeps its own: the loser's icon is lost.
        let winner = merge_with_icons(
            db,
            530,
            Some("https://winner.example/w.png"),
            Some("https://loser.example/l.png"),
        )
        .await;
        assert_eq!(
            winner.icon_url.as_deref(),
            Some("https://winner.example/w.png")
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn write_api_db_asset_lock_does_not_block_edge_inserts() {
        let guard = init_db("write_api_db_asset_lock_edge_inserts").await;
        let conn = guard.client();
        let db = conn.as_ref();
        seed_chains_and_bridge(db, &[1, 100]).await;
        let id = seed_asset(db, None, None, Some(OLD_ICON)).await;

        // An uncommitted icon change holds FOR NO KEY UPDATE on the asset row.
        let icon_tx = conn.begin().await.unwrap();
        set_stats_asset_icon_tx(&icon_tx, id, Some(NEW_ICON), Duration::from_secs(5))
            .await
            .unwrap()
            .expect("asset exists");

        // A maintenance-style insert whose foreign-key check takes FOR KEY
        // SHARE on that row must complete instead of waiting for it.
        let edge_tx = conn.begin().await.unwrap();
        let insert = stats_asset_edges::Entity::insert(stats_asset_edges::ActiveModel {
            src_stats_asset_id: ActiveValue::Set(id),
            dst_stats_asset_id: ActiveValue::Set(id),
            bridge_id: ActiveValue::Set(1),
            src_chain_id: ActiveValue::Set(1),
            dst_chain_id: ActiveValue::Set(100),
            transfers_count: ActiveValue::Set(0),
            cumulative_amount: ActiveValue::Set(BigDecimal::from(0u64)),
            decimals: ActiveValue::Set(Some(5)),
            amount_side: ActiveValue::Set(EdgeAmountSide::Destination),
            ..Default::default()
        })
        .exec(&edge_tx);
        tokio::time::timeout(Duration::from_secs(5), insert)
            .await
            .expect("the edge insert must not wait for the icon change")
            .unwrap();
        edge_tx.commit().await.unwrap();
        icon_tx.commit().await.unwrap();

        assert_eq!(asset(db, id).await.icon_url.as_deref(), Some(NEW_ICON));
        assert_eq!(
            stats_asset_edges::Entity::find()
                .all(db)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn write_api_db_asset_lock_timeout_is_lock_not_available() {
        let guard = init_db("write_api_db_asset_lock_timeout").await;
        let conn = guard.client();
        let db = conn.as_ref();
        let id = seed_asset(db, None, None, Some(OLD_ICON)).await;

        // A FOR UPDATE holder (not what this module takes) conflicts with
        // FOR NO KEY UPDATE.
        let holder = conn.begin().await.unwrap();
        stats_assets::Entity::find_by_id(id)
            .lock_exclusive()
            .one(&holder)
            .await
            .unwrap()
            .expect("asset exists");

        let tx = conn.begin().await.unwrap();
        let err = set_stats_asset_icon_tx(&tx, id, Some(NEW_ICON), Duration::from_millis(200))
            .await
            .err()
            .expect("the lock wait must time out");
        assert!(is_lock_not_available(&err), "unexpected error: {err:?}");
        assert!(!is_lock_not_available(&DbErr::Custom("other".into())));
        tx.rollback().await.unwrap();
        holder.rollback().await.unwrap();

        assert_eq!(asset(db, id).await.icon_url.as_deref(), Some(OLD_ICON));
    }
}
