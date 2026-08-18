// SPDX-License-Identifier: LicenseRef-Blockscout

/// DB read routines for Interchain mode.
use std::collections::{BTreeMap, BTreeSet};

use chrono::{NaiveDateTime, Utc};
use interchain_indexer_entity::{bridge_contracts, bridges, crosschain_messages};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QuerySelect,
    sea_query::Func,
};

use crate::charts::db_interaction::filters::interchain::InterchainFilter;

#[derive(FromQueryResult, Debug)]
struct MinTimestamp {
    min_timestamp: Option<NaiveDateTime>,
}

/// Earliest message timestamp **inside the configured slice**.
///
/// `min` of the message-filtered minimum and the transfer-filtered minimum,
/// because a transfer can satisfy `transfers_condition()` while its own message
/// fails `messages_condition()` — a transfer's token chains need not equal its
/// message's route. Using the message minimum alone would silently truncate the
/// transfer charts' history.
///
/// Two queries and a `min` in Rust rather than a `LEAST` of two subqueries: this
/// runs a handful of times per group update, and it keeps the builders readable.
///
/// Filtering this floor is not cosmetic. It is the start of every batched
/// backfill, so an unfiltered minimum makes a filtered deployment begin at a
/// foreign date and burn a long run of provably empty batches — and on a
/// universal indexer whose history predates the configured slice, that run is
/// the first thing the service does after deployment, unattended.
pub async fn get_min_date_interchain(
    interchain: &DatabaseConnection,
    filter: &InterchainFilter,
) -> Result<NaiveDateTime, DbErr> {
    async fn min_init_timestamp<E: EntityTrait>(
        query: sea_orm::Select<E>,
        interchain: &DatabaseConnection,
    ) -> Result<Option<NaiveDateTime>, DbErr> {
        Ok(query
            .select_only()
            .expr_as(
                Func::min(crosschain_messages::Column::InitTimestamp.into_expr()),
                "min_timestamp",
            )
            .into_model::<MinTimestamp>()
            .one(interchain)
            .await?
            .and_then(|row| row.min_timestamp))
    }

    let messages_min = min_init_timestamp(filter.messages_query(), interchain).await?;
    // the joined query, because `init_timestamp` lives on the message
    let transfers_min = min_init_timestamp(filter.transfers_joined_query(), interchain).await?;

    Ok([messages_min, transfers_min]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or_else(|| Utc::now().naive_utc()))
}

/// The interchain indexer has no block numbers. This slot in the update
/// pipeline — persisted as `chart_data.min_blockscout_block` and compared by
/// `last_accurate_point` — is reused as the *filter fingerprint*: "was the
/// stored history computed under the currently configured filter?", which is
/// exactly the question that comparison answers.
///
/// It used to be the constant `i64::MAX`, which made the channel inert and is
/// what the `TODO: recalculate statistics data when …` comment in
/// `stats-server/src/settings.rs` was about.
///
/// The fingerprint covers only the operator-configured dimensions, never the
/// DB-derived observability horizon's contents — see
/// [`crate::charts::db_interaction::filters::interchain::filter_fingerprint`].
///
/// Kept `async` and `Result`-returning, and kept inside the `match cx.mode`
/// dispatch in [`crate::data_source::kinds::local_db`], per
/// `.memory-bank/rules/database.md`'s mode-dispatching-read-helper convention,
/// even though it no longer touches the database and cannot fail: uniformity at
/// the call site is worth more than removing an `async`.
pub async fn get_min_block_interchain(filter: &InterchainFilter) -> Result<i64, DbErr> {
    Ok(filter.fingerprint)
}

#[derive(FromQueryResult, Debug)]
struct BridgeChainPair {
    bridge_id: i32,
    chain_id: Option<i64>,
}

/// The per-bridge indexed-chain set — the "observability horizon" — derived from
/// the indexer's own `bridges` / `bridge_contracts` tables.
///
/// This mirrors `IndexedChains::configured_pairs` on the indexer side. The
/// indexer builds its set from the in-memory `bridges.json` (ADR-004 Decision 1);
/// stats has no access to that file, so it reads the tables the indexer upserts
/// from it at every startup. The DB-derived set is exact for a steady-state
/// deployment and diverges only under config edits the indexer itself warns
/// about.
///
/// Four rules, each load-bearing:
///
/// - **LEFT JOIN.** A bridge declared with zero contracts must survive as
///   `(b, vec![])`. Dropping it would promote that bridge to the permissive
///   "absent" case, which is the opposite treatment (ADR-004 Decision 5).
/// - **Never filter on `bridges.enabled`.** `upsert_bridges` sets
///   `enabled = false` on every row inside a transaction before re-upserting the
///   configured ones, so `enabled` cannot distinguish "disabled in config" from
///   "removed from config" — and `IndexedChains` deliberately includes disabled
///   bridges.
/// - **De-duplicate chain ids.** `bridge_contracts` is
///   `UNIQUE(bridge_id, chain_id, address, version)`, so one bridge can have
///   several rows per chain.
/// - **Prune to `bridge_ids` when set**, matching
///   `configured_pairs(bridge_ids.as_deref())`. The pruned ids must also leave
///   the permissive arm's `NOT IN` list, which is exactly what pruning the pair
///   list achieves.
///
/// Returns `None` when `bridges` is empty: that is the DB image of an empty
/// bridges config, which the indexer models as `IndexedChains::AllIndexed` and
/// for which `configured_pairs` returns `None`. (`Some(vec![])` restricts
/// nothing either — its permissive arm covers every bridge — but it renders a
/// dead `(TRUE …)` block instead of no predicate, and for messages that block
/// still carries `dst IS NOT NULL`.)
///
/// Pruning to a `bridge_ids` that matches nothing is a *different* case and
/// still returns `Some(vec![])`, exactly as `configured_pairs` does: only
/// `AllIndexed` maps to `None` there. It is unobservable in the result either
/// way — the caller separately ANDs `bridge_id IN (bridge_ids)`, which is
/// already empty — but keeping the two cases distinct keeps this function a
/// faithful mirror rather than an approximation.
pub async fn resolve_only_indexed_by_bridge(
    interchain: &DatabaseConnection,
    bridge_ids: Option<&[i32]>,
) -> Result<Option<Vec<(i32, Vec<i64>)>>, DbErr> {
    let rows = bridges::Entity::find()
        .select_only()
        .column_as(bridges::Column::Id, "bridge_id")
        .column_as(bridge_contracts::Column::ChainId, "chain_id")
        .left_join(bridge_contracts::Entity)
        .into_model::<BridgeChainPair>()
        .all(interchain)
        .await?;

    // `BTreeMap`/`BTreeSet` give both the de-duplication and the (bridge, chain)
    // ordering for free; the indexer sorts `configured_pairs` the same way, so the
    // rendered SQL is identical for identical inputs.
    let mut horizon: BTreeMap<i32, BTreeSet<i64>> = BTreeMap::new();
    for BridgeChainPair {
        bridge_id,
        chain_id,
    } in rows
    {
        // the bridge key is inserted whether or not the LEFT JOIN produced a
        // contract row — that is the whole point of the outer join
        let chains = horizon.entry(bridge_id).or_default();
        if let Some(chain_id) = chain_id {
            chains.insert(chain_id);
        }
    }

    // no bridge at all ⇒ `AllIndexed` ⇒ no restriction. Checked *before* pruning,
    // so that "pruned to nothing" stays the distinct `Some(vec![])` case.
    if horizon.is_empty() {
        return Ok(None);
    }
    if let Some(bridge_ids) = bridge_ids {
        horizon.retain(|bridge_id, _| bridge_ids.contains(bridge_id));
    }

    Ok(Some(
        horizon
            .into_iter()
            .map(|(bridge_id, chains)| (bridge_id, chains.into_iter().collect()))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};

    use super::*;
    use crate::tests::init_db::init_db_interchain;

    /// `chains` / `bridges` / `bridge_contracts` are all FK-linked, so the
    /// reference rows have to exist before the contracts do. `bridges.name` and
    /// `chains.name` are `TEXT NOT NULL UNIQUE`; `bridge_contracts.address` and
    /// `.version` are NOT NULL and take part in the
    /// `UNIQUE (bridge_id, chain_id, address, version)` this test exercises.
    async fn seed(
        interchain: &DatabaseConnection,
        chain_ids: &[i64],
        bridges_rows: &[(i32, bool)],
        contracts: &[(i32, i64, u8, i16)],
    ) {
        for chain_id in chain_ids {
            exec(
                interchain,
                format!("INSERT INTO chains (id, name) VALUES ({chain_id}, 'chain_{chain_id}')"),
            )
            .await;
        }
        for (bridge_id, enabled) in bridges_rows {
            exec(
                interchain,
                format!(
                    "INSERT INTO bridges (id, name, enabled) \
                     VALUES ({bridge_id}, 'bridge_{bridge_id}', {enabled})"
                ),
            )
            .await;
        }
        for (bridge_id, chain_id, address_byte, version) in contracts {
            exec(
                interchain,
                format!(
                    "INSERT INTO bridge_contracts (bridge_id, chain_id, address, version) \
                     VALUES ({bridge_id}, {chain_id}, '\\x{address_byte:02x}'::bytea, {version})"
                ),
            )
            .await;
        }
    }

    async fn exec(interchain: &DatabaseConnection, sql: String) {
        interchain
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                sql,
            ))
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn resolve_horizon_empty_bridges_table_is_unrestricted() {
        let interchain = init_db_interchain("resolve_horizon_empty_bridges_table").await;
        assert_eq!(
            resolve_only_indexed_by_bridge(&interchain, None)
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn resolve_horizon_folds_dedups_and_sorts() {
        let interchain = init_db_interchain("resolve_horizon_folds_dedups_and_sorts").await;
        seed(
            &interchain,
            &[1, 2, 3],
            // bridge 3 is `enabled = false` and must still be listed: `upsert_bridges`
            // disables every row before re-upserting, so `enabled` cannot distinguish
            // "disabled in config" from "removed from config".
            &[(1, true), (2, true), (3, false)],
            &[
                // out of order, and two contracts of bridge 1 on chain 3 —
                // `UNIQUE(bridge_id, chain_id, address, version)` permits that, so the
                // fold has to de-duplicate
                (1, 3, 0xaa, 1),
                (1, 3, 0xbb, 1),
                (1, 3, 0xaa, 2),
                (1, 1, 0xcc, 1),
                (3, 2, 0xdd, 1),
            ],
            // bridge 2 gets no contracts at all
        )
        .await;

        assert_eq!(
            resolve_only_indexed_by_bridge(&interchain, None)
                .await
                .unwrap(),
            Some(vec![
                (1, vec![1, 3]),
                // a contract-less bridge survives as `(b, vec![])` — dropping it would
                // promote it to the permissive "absent" case, the opposite treatment
                (2, vec![]),
                (3, vec![2]),
            ])
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn resolve_horizon_prunes_to_bridge_ids() {
        let interchain = init_db_interchain("resolve_horizon_prunes_to_bridge_ids").await;
        seed(
            &interchain,
            &[1, 2],
            &[(1, true), (2, true)],
            &[(1, 1, 0xaa, 1), (2, 2, 0xbb, 1)],
        )
        .await;

        assert_eq!(
            resolve_only_indexed_by_bridge(&interchain, Some(&[2]))
                .await
                .unwrap(),
            Some(vec![(2, vec![2])])
        );
        // pruning everything away is `Some(vec![])`, not `None`: only an empty
        // `bridges` table means "no per-bridge config at all"
        assert_eq!(
            resolve_only_indexed_by_bridge(&interchain, Some(&[404]))
                .await
                .unwrap(),
            Some(vec![])
        );
    }
}
