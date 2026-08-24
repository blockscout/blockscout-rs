// SPDX-License-Identifier: LicenseRef-Blockscout

//! Paginated chain list with `stats_chains.unique_transfer_users_count` (`/stats/chains`).

use sea_orm::{ConnectionTrait, DatabaseBackend, DbErr, FromQueryResult, Statement, Value};

use crate::{
    pagination::{
        OutputPagination, PaginationDirection, StatsChainsPaginationLogic, StatsChainsSortField,
        StatsSortOrder,
    },
    stats::StatsListQuery,
};

#[derive(Debug, Clone, FromQueryResult)]
pub struct StatsChainListRow {
    pub chain_id: i64,
    pub name: String,
    pub icon_url: Option<String>,
    pub explorer_url: Option<String>,
    pub unique_transfer_users_count: i64,
}

impl StatsChainListRow {
    pub fn marker(&self, direction: PaginationDirection) -> StatsChainsPaginationLogic {
        StatsChainsPaginationLogic {
            direction,
            count: self.unique_transfer_users_count,
            chain_id: self.chain_id,
        }
    }
}

fn forward_order_clause(order: StatsSortOrder) -> &'static str {
    match order {
        StatsSortOrder::Desc => "t.cnt DESC, t.chain_id ASC",
        StatsSortOrder::Asc => "t.cnt ASC, t.chain_id ASC",
    }
}

fn inverse_order_clause(order: StatsSortOrder) -> &'static str {
    match order {
        StatsSortOrder::Desc => "t.cnt ASC, t.chain_id DESC",
        StatsSortOrder::Asc => "t.cnt DESC, t.chain_id DESC",
    }
}

fn cursor_where_next(
    order: StatsSortOrder,
    m: &StatsChainsPaginationLogic,
    p0: usize,
) -> (String, Vec<Value>) {
    let mut vals = Vec::new();
    let c = m.count;
    let id = m.chain_id;
    vals.push(Value::BigInt(Some(c)));
    vals.push(Value::BigInt(Some(id)));
    let p1 = p0 + 1;
    let sql = match order {
        StatsSortOrder::Desc => format!(
            " AND ((t.cnt < ${p0}) OR (t.cnt = ${p0} AND t.chain_id > ${p1}))",
            p0 = p0,
            p1 = p1,
        ),
        StatsSortOrder::Asc => format!(
            " AND ((t.cnt > ${p0}) OR (t.cnt = ${p0} AND t.chain_id > ${p1}))",
            p0 = p0,
            p1 = p1,
        ),
    };
    (sql, vals)
}

fn cursor_where_prev(
    order: StatsSortOrder,
    m: &StatsChainsPaginationLogic,
    p0: usize,
) -> (String, Vec<Value>) {
    let mut vals = Vec::new();
    let c = m.count;
    let id = m.chain_id;
    vals.push(Value::BigInt(Some(c)));
    vals.push(Value::BigInt(Some(id)));
    let p1 = p0 + 1;
    let sql = match order {
        StatsSortOrder::Desc => format!(
            " AND ((t.cnt > ${p0}) OR (t.cnt = ${p0} AND t.chain_id < ${p1}))",
            p0 = p0,
            p1 = p1,
        ),
        StatsSortOrder::Asc => format!(
            " AND ((t.cnt < ${p0}) OR (t.cnt = ${p0} AND t.chain_id < ${p1}))",
            p0 = p0,
            p1 = p1,
        ),
    };
    (sql, vals)
}

fn build_pagination(
    rows: &[StatsChainListRow],
    query_direction: PaginationDirection,
    has_more: bool,
    last_page: bool,
) -> OutputPagination<StatsChainsPaginationLogic> {
    let prev_marker = rows.first().map(|r| r.marker(PaginationDirection::Prev));

    let next_marker = if !last_page && (query_direction == PaginationDirection::Prev || has_more) {
        rows.last().map(|r| r.marker(PaginationDirection::Next))
    } else {
        None
    };

    OutputPagination {
        prev_marker,
        next_marker,
    }
}

/// Which snapshot a `/stats/chains` read draws from.
///
/// `Global` is the exact, globally deduplicated `stats_chains` path — untouched
/// by this task. `Bridges` sums selected bridges' `stats_chains_by_bridge`
/// cells: exact for one bridge, an accepted additive overcount for several (see
/// the module doc and `coding-task-2.md`).
///
/// `bridge_ids` scopes **attribution only** (the count column). It never
/// scopes **which chains appear** — that list is driven by `chain_ids` / `q`
/// / `include_unindexed_chains` exactly as the unfiltered request, so a chain
/// visible without `bridge_ids` stays visible with it, just with its count
/// reduced to what the selected bridges contributed (0 if none did). An
/// earlier revision additionally required a chain to have selected-bridge
/// activity or be in the selected bridges' own current config before it could
/// appear at all — that silently dropped chains indexed only by a *different*
/// bridge (or not configured anywhere, under `include_unindexed_chains =
/// true`) from the bridge-filtered listing, which is not what "scope the
/// count" means. Fixed 2026-08-25 after a live report of exactly this: chains
/// visible with plain `include_unindexed_chains=true` disappearing once
/// `bridge_ids` was added.
pub enum StatsChainsScope<'a> {
    Global,
    Bridges {
        /// Non-empty, sorted, deduplicated.
        bridge_ids: &'a [i32],
    },
}

/// Builds the bridge-scoped `LEFT JOIN` aggregate clause. Placeholder numbers
/// are always derived from `values.len() + 1` at the point each value is
/// pushed, so a predicate appended after this call keeps `$N` numbering
/// contiguous by construction. Pinned by
/// `test_bridge_scope_join_contiguous_with_predicate_appended_after`.
fn build_bridge_scope_join(bridge_ids: &[i32], values: &mut Vec<Value>) -> String {
    let bridge_start = values.len() + 1;
    let bridge_placeholders: Vec<String> = (0..bridge_ids.len())
        .map(|i| format!("${}", bridge_start + i))
        .collect();
    for id in bridge_ids {
        values.push(Value::Int(Some(*id)));
    }

    format!(
        "LEFT JOIN (\n    SELECT chain_id,\n           SUM(unique_transfer_users_count) AS transfer_cnt,\n           SUM(unique_message_users_count) AS message_cnt\n    FROM stats_chains_by_bridge\n    WHERE bridge_id IN ({})\n    GROUP BY chain_id\n) agg ON agg.chain_id = c.id",
        bridge_placeholders.join(", ")
    )
}

pub async fn list_stats_chains(
    db: &impl ConnectionTrait,
    scope: StatsChainsScope<'_>,
    chain_ids: &[i64],
    include_zero_chains: bool,
    indexed_chain_ids: Option<&[i64]>,
    params: StatsListQuery<'_, StatsChainsSortField, StatsChainsPaginationLogic>,
) -> Result<
    (
        Vec<StatsChainListRow>,
        OutputPagination<StatsChainsPaginationLogic>,
    ),
    DbErr,
> {
    let StatsListQuery {
        sort,
        order,
        page_size,
        last_page,
        input_pagination,
        q,
    } = params;

    match sort {
        StatsChainsSortField::UniqueTransferUsersCount => {}
    }

    let limit = page_size.max(1) as i64;
    let fetch = limit.saturating_add(1);

    let query_direction = if last_page {
        PaginationDirection::Prev
    } else {
        input_pagination
            .as_ref()
            .map(|p| p.direction)
            .unwrap_or(PaginationDirection::Next)
    };

    let reverse_results = matches!(query_direction, PaginationDirection::Prev);

    let q_pattern = q.map(|s| {
        let escaped = s
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        format!("%{escaped}%")
    });

    let mut values: Vec<Value> = Vec::new();
    let mut inner_conditions = Vec::new();

    // `Global` keeps the exact pre-existing join/expressions untouched.
    // `Bridges` swaps in the bridge-scoped aggregate; its bridge-id
    // placeholders sit inside the `LEFT JOIN` subquery, textually before every
    // other inner-`WHERE` predicate, so they must be pushed into `values`
    // first too (`coding-task-2.md` item 5 / `database.md`'s placeholder rule).
    let (join_clause, transfer_expr, message_expr) = match scope {
        StatsChainsScope::Global => (
            "LEFT JOIN stats_chains sc ON sc.chain_id = c.id".to_string(),
            "sc.unique_transfer_users_count",
            "sc.unique_message_users_count",
        ),
        StatsChainsScope::Bridges { bridge_ids } => {
            debug_assert!(
                !bridge_ids.is_empty(),
                "StatsChainsScope::Bridges requires a non-empty bridge_ids"
            );
            let join = build_bridge_scope_join(bridge_ids, &mut values);
            (join, "agg.transfer_cnt", "agg.message_cnt")
        }
    };

    if !chain_ids.is_empty() {
        // Placeholder numbers derive from `values.len()`, not a hardcoded `i +
        // 1`: `Bridges` scope already pushed its own bridge-id values above,
        // so this predicate is not always first once a scope precedes it.
        let start = values.len() + 1;
        let placeholders: Vec<String> = (0..chain_ids.len())
            .map(|i| format!("${}", start + i))
            .collect();
        for id in chain_ids {
            values.push(Value::BigInt(Some(*id)));
        }
        inner_conditions.push(format!("c.id IN ({})", placeholders.join(", ")));
    }

    if let Some(pat) = q_pattern {
        let ph = values.len() + 1;
        inner_conditions.push(format!(
            "(c.name ILIKE ${ph} ESCAPE '\\' OR CAST(c.id AS TEXT) ILIKE ${ph} ESCAPE '\\')",
        ));
        values.push(Value::String(Some(Box::new(pat))));
    }

    if !include_zero_chains {
        inner_conditions.push(format!(
            "(COALESCE({transfer_expr}, 0) > 0 OR COALESCE({message_expr}, 0) > 0)"
        ));
    }

    // An empty union means no bridge is configured at all. Restrict nothing in
    // that case: emptying the chain directory because `bridges.json` was emptied
    // is exactly the retroactive reinterpretation ADR-004 Decision 5 forbids, and
    // the startup guard already rejects that config.
    if let Some(ids) = indexed_chain_ids.filter(|ids| !ids.is_empty()) {
        // ANDed with the caller's own `chain_ids` filter: the intersection is
        // the intended semantics.
        let start = values.len() + 1;
        let placeholders: Vec<String> = (0..ids.len()).map(|i| format!("${}", start + i)).collect();
        for id in ids {
            values.push(Value::BigInt(Some(*id)));
        }
        inner_conditions.push(format!("c.id IN ({})", placeholders.join(", ")));
    }

    let inner_where = if inner_conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", inner_conditions.join(" AND "))
    };

    let (where_extra, order_clause, cursor_vals) = if last_page {
        (String::new(), inverse_order_clause(order), Vec::new())
    } else {
        match query_direction {
            PaginationDirection::Next => {
                let ord = forward_order_clause(order);
                if let Some(m) = input_pagination.as_ref() {
                    let p0 = values.len() + 1;
                    let (w, v) = cursor_where_next(order, m, p0);
                    (w, ord, v)
                } else {
                    (String::new(), ord, Vec::new())
                }
            }
            PaginationDirection::Prev => {
                let ord = inverse_order_clause(order);
                if let Some(m) = input_pagination.as_ref() {
                    let p0 = values.len() + 1;
                    let (w, v) = cursor_where_prev(order, m, p0);
                    (w, ord, v)
                } else {
                    (String::new(), ord, Vec::new())
                }
            }
        }
    };

    values.extend(cursor_vals);
    let limit_placeholder = values.len() + 1;
    values.push(Value::BigInt(Some(fetch)));

    let sql = format!(
        r#"
SELECT t.chain_id,
       t.name,
       t.icon_url,
       t.explorer_url,
       t.cnt AS unique_transfer_users_count
FROM (
    SELECT c.id AS chain_id,
           c.name,
           c.icon AS icon_url,
           c.explorer AS explorer_url,
           COALESCE({transfer_expr}, 0)::bigint AS cnt
    FROM chains c
    {join_clause}
    {inner_where}
) t
WHERE TRUE
{where_extra}
ORDER BY {order_clause}
LIMIT ${limit_ph}
"#,
        transfer_expr = transfer_expr,
        join_clause = join_clause,
        inner_where = inner_where,
        where_extra = where_extra,
        order_clause = order_clause,
        limit_ph = limit_placeholder,
    );

    let stmt = Statement::from_sql_and_values(DatabaseBackend::Postgres, sql, values);

    let raw = db.query_all(stmt).await?;
    let mut rows: Vec<StatsChainListRow> = Vec::with_capacity(raw.len());
    for r in raw {
        rows.push(StatsChainListRow::from_query_result(&r, "")?);
    }

    let has_more = rows.len() as i64 > limit;
    if has_more {
        rows.truncate(limit as usize);
    }

    if reverse_results {
        rows.reverse();
    }

    let pagination = build_pagination(&rows, query_direction, has_more, last_page);

    Ok((rows, pagination))
}

#[cfg(test)]
mod tests {
    use super::*;
    use interchain_indexer_entity::chains;
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, DatabaseConnection, EntityTrait};

    use crate::{
        pagination::{PaginationDirection, StatsChainsSortField},
        test_utils::init_db,
    };

    async fn seed_chains(db: &DatabaseConnection, ids: &[i64]) {
        if ids.is_empty() {
            return;
        }
        let models: Vec<chains::ActiveModel> = ids
            .iter()
            .map(|&id| chains::ActiveModel {
                id: Set(id),
                name: Set(format!("chain-{id}")),
                ..Default::default()
            })
            .collect();
        chains::Entity::insert_many(models).exec(db).await.unwrap();
    }

    async fn seed_bridges(db: &DatabaseConnection, ids: &[i32]) {
        if ids.is_empty() {
            return;
        }
        use interchain_indexer_entity::bridges;
        let models: Vec<bridges::ActiveModel> = ids
            .iter()
            .map(|&id| bridges::ActiveModel {
                id: Set(id),
                name: Set(format!("bridge-{id}")),
                enabled: Set(true),
                ..Default::default()
            })
            .collect();
        bridges::Entity::insert_many(models).exec(db).await.unwrap();
    }

    async fn seed_stats_chains_by_bridge(
        db: &DatabaseConnection,
        bridge_id: i32,
        chain_id: i64,
        transfer: i64,
        message: i64,
    ) {
        use interchain_indexer_entity::stats_chains_by_bridge;
        stats_chains_by_bridge::ActiveModel {
            bridge_id: Set(bridge_id),
            chain_id: Set(chain_id),
            unique_transfer_users_count: Set(transfer),
            unique_message_users_count: Set(message),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
    }

    fn default_query(
        page_size: usize,
    ) -> StatsListQuery<'static, StatsChainsSortField, StatsChainsPaginationLogic> {
        StatsListQuery {
            sort: StatsChainsSortField::default(),
            order: StatsSortOrder::Desc,
            page_size,
            last_page: false,
            input_pagination: None,
            q: None,
        }
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_left_join_missing_stats_is_zero() {
        let g = init_db("stats_chains_left_join").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        crate::InterchainDatabase::new(db.clone())
            .upsert_stats_chains(1, 5, 0)
            .await
            .unwrap();

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 2);
        let r1 = rows.iter().find(|r| r.chain_id == 1).unwrap();
        let r2 = rows.iter().find(|r| r.chain_id == 2).unwrap();
        assert_eq!(r1.unique_transfer_users_count, 5);
        assert_eq!(r2.unique_transfer_users_count, 0);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_disabled_omits_missing_and_zero_rows() {
        let g = init_db("stats_chains_disabled_zero_filter").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2, 3]).await;
        let idb = crate::InterchainDatabase::new(db.clone());
        idb.upsert_stats_chains(1, 5, 0).await.unwrap();
        idb.upsert_stats_chains(2, 0, 0).await.unwrap();

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            false,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(
            rows.iter().map(|row| row.chain_id).collect::<Vec<_>>(),
            vec![1]
        );
        assert_eq!(rows[0].unique_transfer_users_count, 5);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_disabled_pagination_keeps_filtered_order() {
        let g = init_db("stats_chains_disabled_pagination").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[10, 11, 12, 13]).await;
        let idb = crate::InterchainDatabase::new(db.clone());
        idb.upsert_stats_chains(10, 10, 0).await.unwrap();
        idb.upsert_stats_chains(11, 0, 0).await.unwrap();
        idb.upsert_stats_chains(12, 10, 0).await.unwrap();

        let (p1, pag1) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            false,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 1,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            p1.iter().map(|row| row.chain_id).collect::<Vec<_>>(),
            vec![10]
        );

        let (p2, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            false,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 1,
                last_page: false,
                input_pagination: pag1.next_marker,
                q: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            p2.iter().map(|row| row.chain_id).collect::<Vec<_>>(),
            vec![12]
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_default_desc_by_count() {
        let g = init_db("stats_chains_desc").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[10, 20, 30]).await;
        let idb = crate::InterchainDatabase::new(db.clone());
        idb.upsert_stats_chains(10, 1, 0).await.unwrap();
        idb.upsert_stats_chains(20, 99, 0).await.unwrap();
        idb.upsert_stats_chains(30, 50, 0).await.unwrap();

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(
            rows.iter().map(|r| r.chain_id).collect::<Vec<_>>(),
            vec![20, 30, 10]
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_asc_order() {
        let g = init_db("stats_chains_asc").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        let idb = crate::InterchainDatabase::new(db.clone());
        idb.upsert_stats_chains(1, 100, 0).await.unwrap();
        idb.upsert_stats_chains(2, 200, 0).await.unwrap();

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Asc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(rows[0].chain_id, 1);
        assert_eq!(rows[1].chain_id, 2);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_tie_breaker_chain_id_asc() {
        let g = init_db("stats_chains_tie").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[5, 3, 7]).await;
        let idb = crate::InterchainDatabase::new(db.clone());
        idb.upsert_stats_chains(5, 42, 0).await.unwrap();
        idb.upsert_stats_chains(3, 42, 0).await.unwrap();
        idb.upsert_stats_chains(7, 42, 0).await.unwrap();

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(
            rows.iter().map(|r| r.chain_id).collect::<Vec<_>>(),
            vec![3, 5, 7]
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_pagination_across_ties() {
        let g = init_db("stats_chains_page_ties").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[100, 101, 102]).await;
        let idb = crate::InterchainDatabase::new(db.clone());
        idb.upsert_stats_chains(100, 10, 0).await.unwrap();
        idb.upsert_stats_chains(101, 10, 0).await.unwrap();
        idb.upsert_stats_chains(102, 99, 0).await.unwrap();

        let (p1, pag1) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 1,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].chain_id, 102);
        let next = pag1.next_marker.expect("next page");
        assert_eq!(next.direction, PaginationDirection::Next);

        let (p2, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 1,
                last_page: false,
                input_pagination: Some(next),
                q: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(p2.len(), 1);
        assert_eq!(p2[0].chain_id, 100);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_filter_by_chain_ids() {
        let g = init_db("stats_chains_filter").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2, 3]).await;
        let idb = crate::InterchainDatabase::new(db.clone());
        idb.upsert_stats_chains(1, 1, 0).await.unwrap();
        idb.upsert_stats_chains(2, 2, 0).await.unwrap();
        idb.upsert_stats_chains(3, 3, 0).await.unwrap();

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[3, 1],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].chain_id, 3);
        assert_eq!(rows[1].chain_id, 1);
    }

    async fn seed_chain_named(db: &DatabaseConnection, id: i64, name: &str) {
        chains::Entity::insert(chains::ActiveModel {
            id: Set(id),
            name: Set(name.to_string()),
            ..Default::default()
        })
        .exec(db)
        .await
        .unwrap();
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_q_filters_by_chain_name() {
        let g = init_db("stats_chains_q_name").await;
        let db = g.client();
        seed_chain_named(db.as_ref(), 1, "FooUniqueBar").await;
        seed_chain_named(db.as_ref(), 2, "Other").await;

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: Some("unique"),
            },
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].chain_id, 1);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_q_filters_by_textual_chain_id() {
        let g = init_db("stats_chains_q_id").await;
        let db = g.client();
        seed_chain_named(db.as_ref(), 43114, "Somewhere").await;
        seed_chain_named(db.as_ref(), 9, "Nine").await;

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: Some("4311"),
            },
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].chain_id, 43114);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_q_malicious_string_is_literal_substring() {
        let g = init_db("stats_chains_q_inj").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2, 3]).await;

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: Some("' OR 1=1 --"),
            },
        )
        .await
        .unwrap();

        assert!(rows.is_empty());
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_q_pagination_preserves_order() {
        let g = init_db("stats_chains_q_page").await;
        let db = g.client();
        seed_chain_named(db.as_ref(), 100, "match-a").await;
        seed_chain_named(db.as_ref(), 101, "match-b").await;
        seed_chain_named(db.as_ref(), 102, "other").await;
        let idb = crate::InterchainDatabase::new(db.clone());
        idb.upsert_stats_chains(100, 10, 0).await.unwrap();
        idb.upsert_stats_chains(101, 10, 0).await.unwrap();
        idb.upsert_stats_chains(102, 99, 0).await.unwrap();

        let (p1, pag1) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 1,
                last_page: false,
                input_pagination: None,
                q: Some("match"),
            },
        )
        .await
        .unwrap();
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].chain_id, 100);
        let next = pag1.next_marker.expect("next");

        let (p2, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 1,
                last_page: false,
                input_pagination: Some(next),
                q: Some("match"),
            },
        )
        .await
        .unwrap();
        assert_eq!(p2.len(), 1);
        assert_eq!(p2[0].chain_id, 101);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_explicit_unique_transfer_users_sort_matches_desc_by_count() {
        let g = init_db("stats_chains_explicit_sort").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[10, 20, 30]).await;
        let idb = crate::InterchainDatabase::new(db.clone());
        idb.upsert_stats_chains(10, 1, 0).await.unwrap();
        idb.upsert_stats_chains(20, 99, 0).await.unwrap();
        idb.upsert_stats_chains(30, 50, 0).await.unwrap();

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::UniqueTransferUsersCount,
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(
            rows.iter().map(|r| r.chain_id).collect::<Vec<_>>(),
            vec![20, 30, 10]
        );
    }

    // --- indexed_chain_ids (coding-task-2b item 2) ---

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_indexed_union_omits_chain_outside_union() {
        let g = init_db("stats_chains_indexed_union_omit").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 100, 999]).await;

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            Some(&[1, 100]),
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();

        let ids: Vec<i64> = rows.iter().map(|r| r.chain_id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&100));
        assert!(
            !ids.contains(&999),
            "unindexed chain must be omitted: {ids:?}"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_indexed_union_none_restricts_nothing() {
        let g = init_db("stats_chains_indexed_union_none").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 999]).await;

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();

        let ids: Vec<i64> = rows.iter().map(|r| r.chain_id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&999));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_indexed_union_empty_restricts_nothing() {
        // `Some(&[])` means no bridge is configured at all, which the
        // permissive-absent-bridge rule (ADR-004 Decision 5) says must restrict
        // nothing -- inverted 2026-07-28, this used to render `FALSE`.
        let g = init_db("stats_chains_indexed_union_empty").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 999]).await;

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            Some(&[]),
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();

        let ids: Vec<i64> = rows.iter().map(|r| r.chain_id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&999));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_indexed_union_intersects_request_chain_ids() {
        let g = init_db("stats_chains_indexed_union_intersect").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 100, 250]).await;

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[1, 250],
            true,
            Some(&[1, 100]),
            StatsListQuery {
                sort: StatsChainsSortField::default(),
                order: StatsSortOrder::Desc,
                page_size: 50,
                last_page: false,
                input_pagination: None,
                q: None,
            },
        )
        .await
        .unwrap();

        // Intersection of request chain_ids {1, 250} and union {1, 100} is {1}.
        assert_eq!(rows.iter().map(|r| r.chain_id).collect::<Vec<_>>(), vec![1]);
    }

    // Closes acceptance criterion 9 (review-2b follow-up 2): `bridged-tokens`
    // already has `bridged_tokens_pagination_unaffected_by_indexed_predicate`;
    // this is the `/stats/chains` analogue, with `indexed_chain_ids` active
    // rather than `None`, mirroring `stats_chains_pagination_across_ties`'s
    // dense-page shape but round-tripping forward twice and back twice to the
    // same first page.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_pagination_round_trip_with_indexed_union_active() {
        let g = init_db("stats_chains_pagination_round_trip_indexed_union").await;
        let db = g.client();
        // 999 is seeded but deliberately left out of the union below: it must
        // never surface on any page of the round trip.
        seed_chains(db.as_ref(), &[100, 101, 102, 999]).await;
        let idb = crate::InterchainDatabase::new(db.clone());
        idb.upsert_stats_chains(100, 10, 0).await.unwrap();
        idb.upsert_stats_chains(101, 10, 0).await.unwrap();
        idb.upsert_stats_chains(102, 99, 0).await.unwrap();
        idb.upsert_stats_chains(999, 99, 0).await.unwrap();

        let union = Some([100i64, 101, 102]);

        let query = |input_pagination| {
            let db = db.clone();
            async move {
                list_stats_chains(
                    db.as_ref(),
                    StatsChainsScope::Global,
                    &[],
                    true,
                    union.as_ref().map(|u| u.as_slice()),
                    StatsListQuery {
                        sort: StatsChainsSortField::default(),
                        order: StatsSortOrder::Desc,
                        page_size: 1,
                        last_page: false,
                        input_pagination,
                        q: None,
                    },
                )
                .await
                .unwrap()
            }
        };

        // Dense pages, one row apiece, ordered desc by count with chain_id-asc
        // tie-break: 102 (99), then 100 and 101 (tied at 10).
        let (p1, pag1) = query(None).await;
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].chain_id, 102);
        let next1 = pag1.next_marker.expect("page 1 has a next page");

        let (p2, pag2) = query(Some(next1)).await;
        assert_eq!(p2.len(), 1);
        assert_eq!(p2[0].chain_id, 100);
        let next2 = pag2.next_marker.expect("page 2 has a next page");

        let (p3, pag3) = query(Some(next2)).await;
        assert_eq!(p3.len(), 1);
        assert_eq!(p3[0].chain_id, 101);
        assert!(
            pag3.next_marker.is_none(),
            "page 3 must be the last page: {:?}",
            pag3.next_marker
        );
        let prev3 = pag3.prev_marker.expect("page 3 has a prev page");

        // Walk back: page 3 -> page 2 -> page 1, landing on the same first row.
        let (p2b, pag2b) = query(Some(prev3)).await;
        assert_eq!(p2b.len(), 1);
        assert_eq!(p2b[0].chain_id, 100);
        let prev2 = pag2b.prev_marker.expect("page 2 has a prev page");

        let (p1b, _) = query(Some(prev2)).await;
        assert_eq!(p1b.len(), 1);
        assert_eq!(
            p1b[0].chain_id, p1[0].chain_id,
            "prev/next round trip must return to the same first page"
        );

        // The unindexed chain 999 (same count as 102, so it would otherwise tie
        // for first place) must never appear across any page of the round trip.
        for page in [&p1, &p2, &p3, &p2b, &p1b] {
            assert!(
                !page.iter().any(|r| r.chain_id == 999),
                "chain 999 is outside the indexed union and must stay hidden: {page:?}"
            );
        }
    }

    // --- build_bridge_scope_join: placeholder contiguity (coding-task-2 item 5) ---

    #[test]
    fn test_bridge_scope_join_contiguous_with_predicate_appended_after() {
        let mut values: Vec<Value> = Vec::new();

        let join = build_bridge_scope_join(&[7, 9], &mut values);
        assert!(join.contains("IN ($1, $2)"), "join was: {join}");
        assert_eq!(values.len(), 2);

        // A predicate appended after the helper-pushed bridge-id group must
        // continue numbering from exactly where it left off. This is the
        // shape `list_stats_chains` relies on for its `chain_ids` / `q` /
        // zero-guard predicates, which all sit textually after this join.
        let next_ph = values.len() + 1;
        let predicate = format!("c.id = ${next_ph}");
        values.push(Value::BigInt(Some(42)));
        assert_eq!(predicate, "c.id = $3");
        assert_eq!(values.len(), 3);
    }

    // --- StatsChainsScope::Bridges: DB-backed read-path coverage (coding-task-2 item 8) ---

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_bridges_scope_single_bridge_is_exact() {
        let g = init_db("stats_chains_bridges_single").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        seed_bridges(db.as_ref(), &[10]).await;
        seed_stats_chains_by_bridge(db.as_ref(), 10, 1, 7, 0).await;

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Bridges { bridge_ids: &[10] },
            &[],
            true,
            None,
            default_query(50),
        )
        .await
        .unwrap();

        let r1 = rows.iter().find(|r| r.chain_id == 1).unwrap();
        assert_eq!(r1.unique_transfer_users_count, 7);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_bridges_scope_disjoint_bridges_sum_correctly() {
        let g = init_db("stats_chains_bridges_disjoint").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1]).await;
        seed_bridges(db.as_ref(), &[10, 20]).await;
        seed_stats_chains_by_bridge(db.as_ref(), 10, 1, 5, 0).await;
        seed_stats_chains_by_bridge(db.as_ref(), 20, 1, 3, 0).await;

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Bridges {
                bridge_ids: &[10, 20],
            },
            &[],
            true,
            None,
            default_query(50),
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].unique_transfer_users_count, 8,
            "disjoint bridges must sum exactly"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_bridges_scope_overlapping_bridges_produce_additive_overcount() {
        // The per-bridge snapshot cannot know the two cells overlap on the
        // same real-world address; summing them is the documented,
        // accepted approximation, not a bug this test tries to fix.
        let g = init_db("stats_chains_bridges_overlap").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1]).await;
        seed_bridges(db.as_ref(), &[10, 20]).await;
        seed_stats_chains_by_bridge(db.as_ref(), 10, 1, 5, 0).await;
        seed_stats_chains_by_bridge(db.as_ref(), 20, 1, 5, 0).await;
        // The exact global snapshot, had it been rebuilt with these two
        // bridges overlapping on one address, would read 5 here — but the
        // filtered sum below is 10, which is exactly the accepted overcount.
        crate::InterchainDatabase::new(db.clone())
            .upsert_stats_chains(1, 5, 0)
            .await
            .unwrap();

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Bridges {
                bridge_ids: &[10, 20],
            },
            &[],
            true,
            None,
            default_query(50),
        )
        .await
        .unwrap();
        assert_eq!(rows[0].unique_transfer_users_count, 10);

        let (global_rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Global,
            &[],
            true,
            None,
            default_query(50),
        )
        .await
        .unwrap();
        assert_eq!(
            global_rows[0].unique_transfer_users_count, 5,
            "the unfiltered path must never be implemented as a sum of bridge rows"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_bridges_scope_zero_activity_chain_shown_only_when_allowed() {
        // Chain 2 has no bridge-10 activity at all (no `stats_chains_by_bridge`
        // row) and no special relationship to bridge 10 — it is simply one of
        // the known chains. `bridge_ids` must never remove a chain from the
        // listing, only zero its count, so this is governed purely by
        // `include_zero_chains`, exactly like the `Global` scope's own
        // zero-chains guard.
        let g = init_db("stats_chains_bridges_zero_row").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        seed_bridges(db.as_ref(), &[10]).await;

        let (hidden, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Bridges { bridge_ids: &[10] },
            &[],
            false,
            None,
            default_query(50),
        )
        .await
        .unwrap();
        assert!(
            !hidden.iter().any(|r| r.chain_id == 2),
            "include_zero_chains=false must omit the zero-activity chain"
        );

        let (shown, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Bridges { bridge_ids: &[10] },
            &[],
            true,
            None,
            default_query(50),
        )
        .await
        .unwrap();
        let row2 = shown.iter().find(|r| r.chain_id == 2).unwrap();
        assert_eq!(row2.unique_transfer_users_count, 0);
    }

    /// Regression for the live bug report (2026-08-25): a chain visible with
    /// plain `include_unindexed_chains=true` (no `bridge_ids`) must stay
    /// visible once `bridge_ids` is added, even though the selected bridge
    /// never touched it and it is not configured for that bridge either —
    /// `bridge_ids` scopes the count, never the chain list. An earlier
    /// revision required selected-bridge activity or configuration before a
    /// chain could appear at all, which silently dropped exactly this case.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_bridges_scope_never_drops_a_chain_the_selected_bridge_never_touched() {
        let g = init_db("stats_chains_bridges_unrelated_chain").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        seed_bridges(db.as_ref(), &[10, 20]).await;
        // Bridge 20 (not selected) is the only one with activity on chain 2.
        seed_stats_chains_by_bridge(db.as_ref(), 20, 2, 999, 0).await;

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Bridges { bridge_ids: &[10] },
            &[],
            true,
            None,
            default_query(50),
        )
        .await
        .unwrap();

        let row2 = rows
            .iter()
            .find(|r| r.chain_id == 2)
            .expect("chain 2 must still appear even though bridge 10 never touched it");
        assert_eq!(
            row2.unique_transfer_users_count, 0,
            "bridge 20's count must never leak into bridge 10's scoped view"
        );
    }

    /// Regression for the placeholder-collision bug this fix also caught:
    /// `chain_ids`' predicate used to hardcode `$1, $2, ...` instead of
    /// deriving from `values.len()`, which collided with the bridge-id
    /// placeholders `Bridges` scope pushes first. Chosen so the collision
    /// produces an observably wrong (not merely coincidentally correct)
    /// result if it regresses: chain_ids requests {2, 3}; the collided
    /// binding would silently drop chain 3.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_bridges_scope_chain_ids_placeholders_stay_correct_after_bridge_ids() {
        let g = init_db("stats_chains_bridges_chain_ids_after_bridge_ids").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2, 3, 4]).await;
        seed_bridges(db.as_ref(), &[10]).await;
        seed_stats_chains_by_bridge(db.as_ref(), 10, 2, 20, 0).await;
        seed_stats_chains_by_bridge(db.as_ref(), 10, 3, 30, 0).await;

        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Bridges { bridge_ids: &[10] },
            &[2, 3],
            true,
            None,
            default_query(50),
        )
        .await
        .unwrap();

        let mut ids: Vec<i64> = rows.iter().map(|r| r.chain_id).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![2, 3], "both requested chain_ids must be present");
        assert_eq!(
            rows.iter()
                .find(|r| r.chain_id == 3)
                .unwrap()
                .unique_transfer_users_count,
            30
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_bridges_scope_unindexed_chain_hidden_unless_opted_in() {
        let g = init_db("stats_chains_bridges_unindexed").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 900]).await;
        seed_bridges(db.as_ref(), &[10, 20]).await;
        // Bridge 10 (selected) has activity on chain 900, which no configured
        // bridge indexes (simulated by omitting 900 from `indexed_chain_ids`).
        seed_stats_chains_by_bridge(db.as_ref(), 10, 900, 4, 0).await;
        // Bridge 20 (not selected) also has activity on chain 900 — it must
        // never leak into the selected-bridge-only count once admitted.
        seed_stats_chains_by_bridge(db.as_ref(), 20, 900, 100, 0).await;

        let (hidden, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Bridges { bridge_ids: &[10] },
            &[],
            true,
            Some(&[1]),
            default_query(50),
        )
        .await
        .unwrap();
        assert!(
            !hidden.iter().any(|r| r.chain_id == 900),
            "globally unindexed chain must be hidden by default"
        );

        let (shown, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Bridges { bridge_ids: &[10] },
            &[],
            true,
            None,
            default_query(50),
        )
        .await
        .unwrap();
        let row900 = shown.iter().find(|r| r.chain_id == 900).unwrap();
        assert_eq!(
            row900.unique_transfer_users_count, 4,
            "admitted chain must still show only the selected bridge's count"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_bridges_scope_configured_chain_absent_from_chains_yields_no_row() {
        let g = init_db("stats_chains_bridges_absent_chain").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1]).await;
        seed_bridges(db.as_ref(), &[10]).await;
        // Chain 999 has no `chains` row at all (for example, removed from the
        // chain directory). Driving the query from `chains` makes this
        // unrepresentable rather than merely absent: it must not fail
        // deserialization (StatsChainListRow.name is non-nullable) and must
        // not appear.
        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Bridges { bridge_ids: &[10] },
            &[],
            true,
            None,
            default_query(50),
        )
        .await
        .unwrap();
        assert!(!rows.iter().any(|r| r.chain_id == 999));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_bridges_scope_composes_filters_with_and() {
        let g = init_db("stats_chains_bridges_composes_and").await;
        let db = g.client();
        seed_chain_named(db.as_ref(), 1, "alpha-match").await;
        seed_chain_named(db.as_ref(), 2, "beta-match").await;
        seed_bridges(db.as_ref(), &[10]).await;
        seed_stats_chains_by_bridge(db.as_ref(), 10, 1, 5, 0).await;
        seed_stats_chains_by_bridge(db.as_ref(), 10, 2, 5, 0).await;

        // bridge_ids selects bridge 10 on both chains; chain_ids narrows to
        // {1}; q further narrows by name; all must compose through AND.
        let (rows, _) = list_stats_chains(
            db.as_ref(),
            StatsChainsScope::Bridges { bridge_ids: &[10] },
            &[1, 2],
            true,
            None,
            StatsListQuery {
                q: Some("alpha"),
                ..default_query(50)
            },
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].chain_id, 1);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_chains_bridges_scope_pagination_round_trip_hides_unselected_bridge() {
        let g = init_db("stats_chains_bridges_pagination_round_trip").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[100, 101, 102]).await;
        seed_bridges(db.as_ref(), &[10, 99]).await;
        // Selected bridge 10: two chains tied at 10, one chain at 99.
        seed_stats_chains_by_bridge(db.as_ref(), 10, 100, 10, 0).await;
        seed_stats_chains_by_bridge(db.as_ref(), 10, 101, 10, 0).await;
        seed_stats_chains_by_bridge(db.as_ref(), 10, 102, 99, 0).await;
        // Unselected bridge 99 has a higher count on chain 100 that must
        // never leak onto any page of the selected-bridge-only listing.
        seed_stats_chains_by_bridge(db.as_ref(), 99, 100, 1_000_000, 0).await;

        let query = |input_pagination| {
            let db = db.clone();
            async move {
                list_stats_chains(
                    db.as_ref(),
                    StatsChainsScope::Bridges { bridge_ids: &[10] },
                    &[],
                    true,
                    None,
                    StatsListQuery {
                        input_pagination,
                        ..default_query(1)
                    },
                )
                .await
                .unwrap()
            }
        };

        let (p1, pag1) = query(None).await;
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].chain_id, 102);
        assert_eq!(p1[0].unique_transfer_users_count, 99);
        let next1 = pag1.next_marker.expect("page 1 has a next page");

        let (p2, pag2) = query(Some(next1)).await;
        assert_eq!(p2.len(), 1);
        assert_eq!(p2[0].chain_id, 100);
        let next2 = pag2.next_marker.expect("page 2 has a next page");

        let (p3, pag3) = query(Some(next2)).await;
        assert_eq!(p3.len(), 1);
        assert_eq!(p3[0].chain_id, 101);
        assert!(pag3.next_marker.is_none(), "page 3 must be the last page");
        let prev3 = pag3.prev_marker.expect("page 3 has a prev page");

        let (p2b, pag2b) = query(Some(prev3)).await;
        assert_eq!(p2b.len(), 1);
        assert_eq!(p2b[0].chain_id, 100);
        let prev2 = pag2b.prev_marker.expect("page 2 has a prev page");

        let (p1b, _) = query(Some(prev2)).await;
        assert_eq!(p1b.len(), 1);
        assert_eq!(p1b[0].chain_id, p1[0].chain_id);

        for page in [&p1, &p2, &p3, &p2b, &p1b] {
            for row in page {
                assert_ne!(
                    row.unique_transfer_users_count, 1_000_000,
                    "unselected bridge 99's count must never leak into a selected-bridge-only page"
                );
            }
        }
    }
}
