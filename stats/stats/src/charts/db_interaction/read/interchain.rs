// SPDX-License-Identifier: LicenseRef-Blockscout

/// DB read routines for Interchain mode.
use std::collections::{BTreeMap, BTreeSet};

use chrono::{NaiveDateTime, Utc};
use interchain_indexer_entity::{bridge_contracts, bridges, crosschain_messages};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, FromQueryResult, QuerySelect,
    QueryTrait, sea_query::Func,
};

use crate::{
    charts::db_interaction::filters::interchain::InterchainFilter, data_source::UpdateContext,
};

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
/// Two queries and a `min` in Rust rather than a `LEAST` of two subqueries: it
/// keeps the builders readable, and both results are memoised in
/// [`UpdateContext::cache`], so the pair runs once per group update however many
/// charts ask for the floor. That matters because the transfer query joins the
/// whole `crosschain_transfers` table and the horizon disjunction has no index
/// support, so an unmemoised floor was a sequential scan per chart — during
/// exactly the full rebuild a filter change triggers. The cache is per group
/// update, not per cycle, so a rebuild still pays one pair per group.
///
/// Filtering this floor is not cosmetic. It is the start of every batched
/// backfill, so an unfiltered minimum makes a filtered deployment begin at a
/// foreign date and burn a long run of provably empty batches — and on a
/// universal indexer whose history predates the configured slice, that run is
/// the first thing the service does after deployment, unattended.
pub async fn get_min_date_interchain(cx: &UpdateContext<'_>) -> Result<NaiveDateTime, DbErr> {
    async fn min_init_timestamp<E: EntityTrait>(
        query: sea_orm::Select<E>,
        cx: &UpdateContext<'_>,
    ) -> Result<Option<NaiveDateTime>, DbErr> {
        let query = query.select_only().expr_as(
            Func::min(crosschain_messages::Column::InitTimestamp.into_expr()),
            "min_timestamp",
        );
        // `UpdateCache` keys on the statement text, which encodes the whole
        // predicate — including the horizon resolved for this cycle — so the key is
        // complete for the cache's lifetime even though the filter *fingerprint*
        // deliberately excludes the horizon's contents.
        let statement = query.build(DbBackend::Postgres);
        if let Some(cached) = cx.cache.get::<Option<NaiveDateTime>>(&statement).await {
            return Ok(cached);
        }
        let min = query
            .into_model::<MinTimestamp>()
            .one(cx.indexer_db)
            .await?
            .and_then(|row| row.min_timestamp);
        cx.cache.insert(&statement, min).await;
        Ok(min)
    }

    let filter = &cx.interchain_filter;
    let messages_min = min_init_timestamp(filter.messages_query(), cx).await?;
    // the joined query, because `init_timestamp` lives on the message
    let transfers_min = min_init_timestamp(filter.transfers_joined_query(), cx).await?;

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
/// Always returns a list, never "no restriction" — including for an empty
/// `bridges` table, which yields `vec![]`. That is what `configured_pairs`
/// does: the indexer's only production construction is
/// `IndexedChains::from_bridges` (`interchain-indexer-server/src/server.rs`),
/// which builds `PerBridge` even from an empty iterator, and its startup guard
/// (`ensure!(bridges.is_empty() || pair_count() > 0)`) deliberately *allows* the
/// no-bridges config through. So `AllIndexed` — the variant whose
/// `configured_pairs` returns `None` — is reachable only in the indexer's own
/// tests and embedders, never in a deployment stats could be reading.
///
/// The distinction is not cosmetic: for messages, an empty pair list renders
/// `(TRUE AND dst IS NOT NULL)` while no list at all renders nothing, so the two
/// disagree about a message with an unknown destination. Under the schema's
/// foreign keys an empty `bridges` table implies no messages, which makes them
/// result-equivalent today; returning the empty list keeps them equivalent by
/// construction instead of by that invariant.
///
/// "Pruned to nothing" by `bridge_ids` returns `vec![]` for the same reason, and
/// is unobservable either way — the caller separately ANDs
/// `bridge_id IN (bridge_ids)`, which is already empty.
pub async fn resolve_only_indexed_by_bridge(
    interchain: &DatabaseConnection,
    bridge_ids: Option<&[i32]>,
) -> Result<Vec<(i32, Vec<i64>)>, DbErr> {
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

    if let Some(bridge_ids) = bridge_ids {
        horizon.retain(|bridge_id, _| bridge_ids.contains(bridge_id));
    }

    // A bridge present in `bridges` with no `bridge_contracts` rows is the most
    // restrictive entry the horizon can produce: the shared predicate renders its
    // arm as `1 = 2`, excluding every one of that bridge's rows. That is the
    // intended treatment of a bridge configured to observe nothing (ADR-004
    // Decision 5), and this resolver cannot tell it apart from a bridge whose
    // contracts are simply not written yet — the indexer upserts `bridges` and
    // `bridge_contracts` in two separate calls (`server.rs`), so a cycle landing
    // between them sees exactly this shape.
    //
    // The difference matters because a cycle that reads the transient shape writes
    // under-counted points, and nothing later repairs them: the fingerprint
    // deliberately excludes the horizon's contents, so the next cycle sees no
    // mismatch and only updates forward from `last_accurate_point`. Warn rather
    // than second-guess the data — narrowing here is the behaviour that matches
    // the indexer's own API, and a stats-side override would break the parity this
    // filter exists for.
    let observing_nothing: Vec<i32> = horizon
        .iter()
        .filter(|(_, chains)| chains.is_empty())
        .map(|(bridge_id, _)| *bridge_id)
        .collect();
    if !observing_nothing.is_empty() {
        tracing::warn!(
            bridges =? observing_nothing,
            "bridges present in `bridges` with no `bridge_contracts` rows: every row of \
             theirs is excluded from the interchain charts this cycle. Expected if they are \
             configured to observe nothing; if instead the indexer is mid-startup between \
             its `bridges` and `bridge_contracts` upserts, points written this cycle are \
             under-counted and will not be recomputed on their own — force a full update \
             once the contracts are in"
        );
    }

    Ok(horizon
        .into_iter()
        .map(|(bridge_id, chains)| (bridge_id, chains.into_iter().collect()))
        .collect())
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};

    use super::*;
    use crate::tests::{
        init_db::init_db_interchain,
        mock_interchain::{fill_mock_interchain_data, mock_interchain_horizon},
        point_construction::dt,
    };

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

    /// The shared fixture's tables must resolve to the horizon its constant
    /// advertises.
    ///
    /// Around ten `*_horizon` chart tests pass [`mock_interchain_horizon`] in
    /// explicitly, so the whole horizon dimension is otherwise asserted against a
    /// hand-written literal rather than against the resolver the service actually
    /// runs. Edit the fixture's `bridge_contracts` rows — give the second bridge a
    /// contract, drop a chain from `MOCK_BRIDGE_CONTRACT_CHAIN_IDS` — and every one
    /// of those tests would keep passing while asserting values for a horizon the
    /// fixture no longer produces. The three `resolve_horizon_*` tests above seed
    /// their own rows, so they do not close that gap.
    ///
    /// This is also the only place the production default path is exercised:
    /// `include_unindexed_chains = false` with the horizon read from the DB rather
    /// than injected.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn fixture_tables_resolve_to_mock_interchain_horizon() {
        let interchain = init_db_interchain("fixture_resolves_to_mock_horizon").await;
        fill_mock_interchain_data(&interchain, dt("2023-03-01T00:00:00").date()).await;
        assert_eq!(
            resolve_only_indexed_by_bridge(&interchain, None)
                .await
                .unwrap(),
            mock_interchain_horizon()
        );
    }

    /// An empty `bridges` table is the DB image of a no-bridges config, which the
    /// indexer's startup guard allows and turns into `PerBridge(empty)` — so
    /// `configured_pairs` gives an empty list, not "no restriction". Mirroring
    /// that matters for the one row type the two would disagree about: a message
    /// with a NULL destination, which the empty list excludes.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn resolve_horizon_empty_bridges_table_mirrors_per_bridge_empty() {
        let interchain = init_db_interchain("resolve_horizon_empty_bridges_table").await;
        assert_eq!(
            resolve_only_indexed_by_bridge(&interchain, None)
                .await
                .unwrap(),
            vec![]
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
            vec![
                (1, vec![1, 3]),
                // a contract-less bridge survives as `(b, vec![])` — dropping it would
                // promote it to the permissive "absent" case, the opposite treatment
                (2, vec![]),
                (3, vec![2]),
            ]
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
            vec![(2, vec![2])]
        );
        // pruning everything away leaves an empty list, which is also what an
        // empty `bridges` table gives — neither is "no restriction"
        assert_eq!(
            resolve_only_indexed_by_bridge(&interchain, Some(&[404]))
                .await
                .unwrap(),
            vec![]
        );
    }
}
