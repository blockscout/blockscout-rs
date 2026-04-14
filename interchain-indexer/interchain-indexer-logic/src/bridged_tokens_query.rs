//! Aggregated bridged-token stats per `stats_asset` for a chain (`/stats/bridged-tokens`).

use sea_orm::{ConnectionTrait, DatabaseBackend, DbErr, FromQueryResult, Statement, Value};

use crate::pagination::{
    BridgedTokensPaginationLogic, BridgedTokensSortField, OutputPagination, PaginationDirection,
    StatsSortOrder,
};

#[derive(Debug, Clone, FromQueryResult)]
pub struct BridgedTokenAggDbRow {
    pub stats_asset_id: i64,
    pub input_transfers_count: i64,
    pub output_transfers_count: i64,
    pub total_transfers_count: i64,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub icon_url: Option<String>,
    pub name_blank: i32,
    pub name_sort: String,
}

impl BridgedTokenAggDbRow {
    pub fn marker(
        &self,
        direction: PaginationDirection,
        sort: BridgedTokensSortField,
    ) -> BridgedTokensPaginationLogic {
        let name_blank = self.name_blank != 0;
        let count = match sort {
            BridgedTokensSortField::Name => 0,
            BridgedTokensSortField::InputTransfers => self.input_transfers_count,
            BridgedTokensSortField::OutputTransfers => self.output_transfers_count,
            BridgedTokensSortField::TotalTransfers => self.total_transfers_count,
        };

        BridgedTokensPaginationLogic {
            direction,
            stats_asset_id: self.stats_asset_id,
            name_blank,
            name_sort: if name_blank {
                String::new()
            } else {
                self.name_sort.clone()
            },
            count,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BridgedTokenLinkEnriched {
    pub chain_id: i64,
    pub token_address: Vec<u8>,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub icon_url: Option<String>,
    pub decimals: Option<i16>,
}

fn forward_order_clause(sort: BridgedTokensSortField, order: StatsSortOrder) -> &'static str {
    match (sort, order) {
        (BridgedTokensSortField::Name, StatsSortOrder::Asc) => {
            "a.name_blank ASC, a.name_sort ASC, a.stats_asset_id ASC"
        }
        (BridgedTokensSortField::Name, StatsSortOrder::Desc) => {
            "a.name_blank ASC, a.name_sort DESC, a.stats_asset_id DESC"
        }
        (BridgedTokensSortField::InputTransfers, StatsSortOrder::Asc) => {
            "a.input_transfers_count ASC, a.stats_asset_id ASC"
        }
        (BridgedTokensSortField::InputTransfers, StatsSortOrder::Desc) => {
            "a.input_transfers_count DESC, a.stats_asset_id DESC"
        }
        (BridgedTokensSortField::OutputTransfers, StatsSortOrder::Asc) => {
            "a.output_transfers_count ASC, a.stats_asset_id ASC"
        }
        (BridgedTokensSortField::OutputTransfers, StatsSortOrder::Desc) => {
            "a.output_transfers_count DESC, a.stats_asset_id DESC"
        }
        (BridgedTokensSortField::TotalTransfers, StatsSortOrder::Asc) => {
            "a.total_transfers_count ASC, a.stats_asset_id ASC"
        }
        (BridgedTokensSortField::TotalTransfers, StatsSortOrder::Desc) => {
            "a.total_transfers_count DESC, a.stats_asset_id DESC"
        }
    }
}

fn inverse_order_clause(sort: BridgedTokensSortField, order: StatsSortOrder) -> &'static str {
    match (sort, order) {
        (BridgedTokensSortField::Name, StatsSortOrder::Asc) => {
            "a.name_blank DESC, a.name_sort DESC, a.stats_asset_id DESC"
        }
        (BridgedTokensSortField::Name, StatsSortOrder::Desc) => {
            "a.name_blank DESC, a.name_sort ASC, a.stats_asset_id ASC"
        }
        (BridgedTokensSortField::InputTransfers, StatsSortOrder::Asc) => {
            "a.input_transfers_count DESC, a.stats_asset_id DESC"
        }
        (BridgedTokensSortField::InputTransfers, StatsSortOrder::Desc) => {
            "a.input_transfers_count ASC, a.stats_asset_id ASC"
        }
        (BridgedTokensSortField::OutputTransfers, StatsSortOrder::Asc) => {
            "a.output_transfers_count DESC, a.stats_asset_id DESC"
        }
        (BridgedTokensSortField::OutputTransfers, StatsSortOrder::Desc) => {
            "a.output_transfers_count ASC, a.stats_asset_id ASC"
        }
        (BridgedTokensSortField::TotalTransfers, StatsSortOrder::Asc) => {
            "a.total_transfers_count DESC, a.stats_asset_id DESC"
        }
        (BridgedTokensSortField::TotalTransfers, StatsSortOrder::Desc) => {
            "a.total_transfers_count ASC, a.stats_asset_id ASC"
        }
    }
}

fn count_column(sort: BridgedTokensSortField) -> &'static str {
    match sort {
        BridgedTokensSortField::Name => "a.input_transfers_count", // unused for name cursor
        BridgedTokensSortField::InputTransfers => "a.input_transfers_count",
        BridgedTokensSortField::OutputTransfers => "a.output_transfers_count",
        BridgedTokensSortField::TotalTransfers => "a.total_transfers_count",
    }
}

/// `Next` cursor: rows strictly after `m` in forward display order.
fn cursor_where_next(
    sort: BridgedTokensSortField,
    order: StatsSortOrder,
    m: &BridgedTokensPaginationLogic,
    p0: usize,
) -> (String, Vec<Value>) {
    let mut vals = Vec::new();
    let sql = match sort {
        BridgedTokensSortField::Name => {
            let b = m.name_blank as i32;
            let n = m.name_sort.clone();
            let id = m.stats_asset_id;
            vals.push(Value::BigInt(Some(i64::from(b))));
            vals.push(Value::String(Some(Box::new(n))));
            vals.push(Value::BigInt(Some(id)));
            let p1 = p0 + 1;
            let p2 = p0 + 2;
            match order {
                StatsSortOrder::Asc => format!(
                    " AND ((a.name_blank > ${p0}) OR (a.name_blank = ${p0} AND (a.name_sort > ${p1} OR (a.name_sort = ${p1} AND a.stats_asset_id > ${p2}))))",
                    p0 = p0,
                    p1 = p1,
                    p2 = p2,
                ),
                StatsSortOrder::Desc => format!(
                    " AND ((a.name_blank > ${p0}) OR (a.name_blank = ${p0} AND (a.name_sort < ${p1} OR (a.name_sort = ${p1} AND a.stats_asset_id < ${p2}))))",
                    p0 = p0,
                    p1 = p1,
                    p2 = p2,
                ),
            }
        }
        BridgedTokensSortField::InputTransfers
        | BridgedTokensSortField::OutputTransfers
        | BridgedTokensSortField::TotalTransfers => {
            let col = count_column(sort);
            let c = m.count;
            let id = m.stats_asset_id;
            vals.push(Value::BigInt(Some(c)));
            vals.push(Value::BigInt(Some(id)));
            let p1 = p0 + 1;
            match order {
                StatsSortOrder::Asc => format!(
                    " AND (({col} > ${p0}) OR ({col} = ${p0} AND a.stats_asset_id > ${p1}))",
                    col = col,
                    p0 = p0,
                    p1 = p1,
                ),
                StatsSortOrder::Desc => format!(
                    " AND (({col} < ${p0}) OR ({col} = ${p0} AND a.stats_asset_id < ${p1}))",
                    col = col,
                    p0 = p0,
                    p1 = p1,
                ),
            }
        }
    };
    (sql, vals)
}

/// `Prev` cursor: rows strictly before `m` (first row of the client's current page) in display order.
fn cursor_where_prev(
    sort: BridgedTokensSortField,
    order: StatsSortOrder,
    m: &BridgedTokensPaginationLogic,
    p0: usize,
) -> (String, Vec<Value>) {
    let mut vals = Vec::new();
    let sql = match sort {
        BridgedTokensSortField::Name => {
            let b = m.name_blank as i32;
            let n = m.name_sort.clone();
            let id = m.stats_asset_id;
            vals.push(Value::BigInt(Some(i64::from(b))));
            vals.push(Value::String(Some(Box::new(n))));
            vals.push(Value::BigInt(Some(id)));
            let p1 = p0 + 1;
            let p2 = p0 + 2;
            match order {
                StatsSortOrder::Asc => format!(
                    " AND ((a.name_blank < ${p0}) OR (a.name_blank = ${p0} AND (a.name_sort < ${p1} OR (a.name_sort = ${p1} AND a.stats_asset_id < ${p2}))))",
                    p0 = p0,
                    p1 = p1,
                    p2 = p2,
                ),
                StatsSortOrder::Desc => format!(
                    " AND ((a.name_blank < ${p0}) OR (a.name_blank = ${p0} AND (a.name_sort > ${p1} OR (a.name_sort = ${p1} AND a.stats_asset_id > ${p2}))))",
                    p0 = p0,
                    p1 = p1,
                    p2 = p2,
                ),
            }
        }
        BridgedTokensSortField::InputTransfers
        | BridgedTokensSortField::OutputTransfers
        | BridgedTokensSortField::TotalTransfers => {
            let col = count_column(sort);
            let c = m.count;
            let id = m.stats_asset_id;
            vals.push(Value::BigInt(Some(c)));
            vals.push(Value::BigInt(Some(id)));
            let p1 = p0 + 1;
            match order {
                StatsSortOrder::Asc => format!(
                    " AND (({col} < ${p0}) OR ({col} = ${p0} AND a.stats_asset_id < ${p1}))",
                    col = col,
                    p0 = p0,
                    p1 = p1,
                ),
                StatsSortOrder::Desc => format!(
                    " AND (({col} > ${p0}) OR ({col} = ${p0} AND a.stats_asset_id > ${p1}))",
                    col = col,
                    p0 = p0,
                    p1 = p1,
                ),
            }
        }
    };
    (sql, vals)
}

fn build_pagination_from_bridged_tokens(
    rows: &[BridgedTokenAggDbRow],
    sort: BridgedTokensSortField,
    query_direction: PaginationDirection,
    has_more: bool,
    last_page: bool,
) -> OutputPagination<BridgedTokensPaginationLogic> {
    let prev_marker = rows
        .first()
        .map(|r| r.marker(PaginationDirection::Prev, sort));

    let next_marker = if !last_page && (query_direction == PaginationDirection::Prev || has_more) {
        rows.last()
            .map(|r| r.marker(PaginationDirection::Next, sort))
    } else {
        None
    };

    OutputPagination {
        prev_marker,
        next_marker,
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn list_bridged_token_stats_for_chain(
    db: &impl ConnectionTrait,
    chain_id: i64,
    sort: BridgedTokensSortField,
    order: StatsSortOrder,
    page_size: usize,
    last_page: bool,
    input_pagination: Option<BridgedTokensPaginationLogic>,
    q: Option<&str>,
) -> Result<
    (
        Vec<BridgedTokenAggDbRow>,
        OutputPagination<BridgedTokensPaginationLogic>,
    ),
    DbErr,
> {
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

    let q_pattern = q.map(|s| format!("%{s}%"));
    let q_offset: usize = if q_pattern.is_some() { 1 } else { 0 };

    let (where_extra, order_clause, cursor_vals) = if last_page {
        let inv = inverse_order_clause(sort, order);
        (String::new(), inv, Vec::new())
    } else {
        match query_direction {
            PaginationDirection::Next => {
                let ord = forward_order_clause(sort, order);
                if let Some(m) = input_pagination.as_ref() {
                    let p0 = 2 + q_offset;
                    let (w, v) = cursor_where_next(sort, order, m, p0);
                    (w, ord, v)
                } else {
                    (String::new(), ord, Vec::new())
                }
            }
            PaginationDirection::Prev => {
                let ord = inverse_order_clause(sort, order);
                if let Some(m) = input_pagination.as_ref() {
                    let p0 = 2 + q_offset;
                    let (w, v) = cursor_where_prev(sort, order, m, p0);
                    (w, ord, v)
                } else {
                    (String::new(), ord, Vec::new())
                }
            }
        }
    };

    let mut all_values = vec![Value::BigInt(Some(chain_id))];
    let mut name_filter_sql = String::new();
    if let Some(pat) = q_pattern {
        let ph = all_values.len() + 1;
        name_filter_sql = format!(" AND a.name ILIKE ${ph}");
        all_values.push(Value::String(Some(Box::new(pat))));
    }
    all_values.extend(cursor_vals);
    let limit_placeholder = all_values.len() + 1;
    all_values.push(Value::BigInt(Some(fetch)));

    let sql = format!(
        r#"
SELECT a.stats_asset_id,
       a.input_transfers_count,
       a.output_transfers_count,
       a.total_transfers_count,
       a.name,
       a.symbol,
       a.icon_url,
       a.name_blank,
       a.name_sort
FROM (
    SELECT agg.stats_asset_id,
           agg.input_transfers_count,
           agg.output_transfers_count,
           (agg.input_transfers_count + agg.output_transfers_count)::bigint AS total_transfers_count,
           s.name,
           s.symbol,
           s.icon_url,
           (CASE WHEN s.name IS NULL OR btrim(s.name) = '' THEN 1 ELSE 0 END)::int AS name_blank,
           COALESCE(s.name, '') AS name_sort
    FROM (
        SELECT stats_asset_id,
               COALESCE(SUM(CASE WHEN dst_chain_id = $1 THEN transfers_count ELSE 0 END), 0)::bigint AS input_transfers_count,
               COALESCE(SUM(CASE WHEN src_chain_id = $1 THEN transfers_count ELSE 0 END), 0)::bigint AS output_transfers_count
        FROM stats_asset_edges
        WHERE src_chain_id = $1 OR dst_chain_id = $1
        GROUP BY stats_asset_id
    ) agg
    INNER JOIN stats_assets s ON s.id = agg.stats_asset_id
) a
WHERE TRUE
{name_filter}{where_extra}
ORDER BY {order_clause}
LIMIT ${limit_ph}
"#,
        name_filter = name_filter_sql,
        where_extra = where_extra,
        order_clause = order_clause,
        limit_ph = limit_placeholder,
    );

    let stmt = Statement::from_sql_and_values(DatabaseBackend::Postgres, sql, all_values);

    let raw = db.query_all(stmt).await?;
    let mut rows: Vec<BridgedTokenAggDbRow> = Vec::with_capacity(raw.len());
    for r in raw {
        rows.push(BridgedTokenAggDbRow::from_query_result(&r, "")?);
    }

    let has_more = rows.len() as i64 > limit;
    if has_more {
        rows.truncate(limit as usize);
    }

    if reverse_results {
        rows.reverse();
    }

    let pagination =
        build_pagination_from_bridged_tokens(&rows, sort, query_direction, has_more, last_page);

    Ok((rows, pagination))
}

pub async fn fetch_bridged_token_items_for_assets(
    db: &impl ConnectionTrait,
    asset_ids: &[i64],
) -> Result<std::collections::HashMap<i64, Vec<BridgedTokenLinkEnriched>>, DbErr> {
    use std::collections::HashMap;

    if asset_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let placeholders: Vec<String> = (1..=asset_ids.len()).map(|i| format!("${}", i)).collect();
    let sql = format!(
        r#"
SELECT sat.stats_asset_id,
       sat.chain_id,
       sat.token_address,
       t.name AS token_name,
       t.symbol AS token_symbol,
       t.token_icon AS token_icon,
       t.decimals AS token_decimals
FROM stats_asset_tokens sat
LEFT JOIN tokens t ON t.chain_id = sat.chain_id AND t.address = sat.token_address
WHERE sat.stats_asset_id IN ({})
ORDER BY sat.stats_asset_id, sat.chain_id, sat.token_address
"#,
        placeholders.join(", ")
    );

    let vals: Vec<Value> = asset_ids
        .iter()
        .map(|id| Value::BigInt(Some(*id)))
        .collect();
    let stmt = Statement::from_sql_and_values(DatabaseBackend::Postgres, sql, vals);

    let raw = db.query_all(stmt).await?;
    let mut map: HashMap<i64, Vec<BridgedTokenLinkEnriched>> = HashMap::new();
    for r in raw {
        let aid: i64 = r.try_get("", "stats_asset_id")?;
        let chain_id: i64 = r.try_get("", "chain_id")?;
        let token_address: Vec<u8> = r.try_get("", "token_address")?;
        let name: Option<String> = r.try_get("", "token_name").ok();
        let symbol: Option<String> = r.try_get("", "token_symbol").ok();
        let icon: Option<String> = r.try_get("", "token_icon").ok();
        let decimals: Option<i16> = r.try_get("", "token_decimals").ok();
        map.entry(aid).or_default().push(BridgedTokenLinkEnriched {
            chain_id,
            token_address,
            name,
            symbol,
            icon_url: icon,
            decimals,
        });
    }
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use interchain_indexer_entity::{
        chains, sea_orm_active_enums::EdgeAmountSide, stats_asset_edges, stats_asset_tokens,
        stats_assets, tokens,
    };
    use sea_orm::{ActiveValue::Set, DatabaseConnection, EntityTrait, prelude::BigDecimal};

    use crate::{pagination::PaginationDirection, test_utils::init_db};

    async fn seed_chains(db: &DatabaseConnection, ids: &[i64]) {
        if ids.is_empty() {
            return;
        }
        let models: Vec<chains::ActiveModel> = ids
            .iter()
            .map(|&id| chains::ActiveModel {
                id: Set(id),
                name: Set(format!("test-chain-{id}")),
                ..Default::default()
            })
            .collect();
        chains::Entity::insert_many(models).exec(db).await.unwrap();
    }

    async fn seed_asset_edges(
        db: &DatabaseConnection,
        name: Option<String>,
        edges: Vec<(i64, i64, i64)>,
    ) -> i64 {
        let id = stats_assets::Entity::insert(stats_assets::ActiveModel {
            name: Set(name),
            ..Default::default()
        })
        .exec_with_returning(db)
        .await
        .unwrap()
        .id;
        for (src, dst, cnt) in edges {
            stats_asset_edges::Entity::insert(stats_asset_edges::ActiveModel {
                stats_asset_id: Set(id),
                src_chain_id: Set(src),
                dst_chain_id: Set(dst),
                transfers_count: Set(cnt),
                cumulative_amount: Set(BigDecimal::from(0u64)),
                amount_side: Set(EdgeAmountSide::Source),
                ..Default::default()
            })
            .exec(db)
            .await
            .unwrap();
        }
        id
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn bridged_tokens_aggregation_input_output_total() {
        let g = init_db("bridged_tokens_aggregation_io").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2, 3, 9, 99, 100]).await;
        // Chain 1: inbound from 2 (dst=1), outbound to 3 (src=1)
        let _a1 = seed_asset_edges(
            db.as_ref(),
            Some("Alpha".into()),
            vec![(2, 1, 3), (1, 3, 5)],
        )
        .await;
        // Inbound-only on chain 1
        let _a2 = seed_asset_edges(db.as_ref(), Some("Beta".into()), vec![(9, 1, 7)]).await;
        // Unrelated to chain 1
        let _a3 = seed_asset_edges(db.as_ref(), Some("Gamma".into()), vec![(99, 100, 1000)]).await;

        let (rows, _) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            50,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 2);
        let alpha = rows
            .iter()
            .find(|r| r.name.as_deref() == Some("Alpha"))
            .unwrap();
        assert_eq!(alpha.input_transfers_count, 3);
        assert_eq!(alpha.output_transfers_count, 5);
        assert_eq!(alpha.total_transfers_count, 8);
        let beta = rows
            .iter()
            .find(|r| r.name.as_deref() == Some("Beta"))
            .unwrap();
        assert_eq!(beta.input_transfers_count, 7);
        assert_eq!(beta.output_transfers_count, 0);
        assert_eq!(beta.total_transfers_count, 7);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn bridged_tokens_sort_by_counts_stable_id() {
        let g = init_db("bridged_tokens_sort_counts").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2, 3]).await;
        let id_lo = seed_asset_edges(db.as_ref(), Some("Same".into()), vec![(1, 2, 10)]).await;
        let id_hi = seed_asset_edges(db.as_ref(), Some("Same".into()), vec![(1, 3, 10)]).await;
        assert!(id_lo < id_hi);

        let (rows, _) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::OutputTransfers,
            StatsSortOrder::Desc,
            50,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].stats_asset_id, id_hi);
        assert_eq!(rows[1].stats_asset_id, id_lo);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn bridged_tokens_name_sort_nulls_and_empty_last() {
        let g = init_db("bridged_tokens_name_nulls").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2, 3, 4]).await;
        let _with_name = seed_asset_edges(db.as_ref(), Some("Zed".into()), vec![(1, 2, 1)]).await;
        let _blank = seed_asset_edges(db.as_ref(), None, vec![(1, 3, 1)]).await;
        let _empty = seed_asset_edges(db.as_ref(), Some("   ".into()), vec![(1, 4, 1)]).await;

        let (rows, _) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            50,
            false,
            None,
            None,
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].name.as_deref(), Some("Zed"));
        // NULL then whitespace-only both sort after non-blank; tie-break by stats_asset_id
        let tail: Vec<_> = rows[1..].iter().map(|r| r.stats_asset_id).collect();
        assert!(tail[0] < tail[1]);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn bridged_tokens_pagination_next_matches_prev() {
        let g = init_db("bridged_tokens_page").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        for nm in ["A", "B", "C"] {
            seed_asset_edges(db.as_ref(), Some(nm.into()), vec![(1, 2, 1)]).await;
        }

        let (p1, pag1) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            1,
            false,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].name.as_deref(), Some("A"));
        let next_tok = pag1.next_marker.expect("next");
        assert_eq!(next_tok.direction, PaginationDirection::Next);

        let (p2, pag2) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            1,
            false,
            Some(next_tok),
            None,
        )
        .await
        .unwrap();
        assert_eq!(p2[0].name.as_deref(), Some("B"));
        let prev_tok = pag2.prev_marker.expect("prev");
        assert_eq!(prev_tok.direction, PaginationDirection::Prev);

        let (p1b, _) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            1,
            false,
            Some(prev_tok),
            None,
        )
        .await
        .unwrap();
        assert_eq!(p1b[0].name.as_deref(), Some("A"));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn bridged_tokens_last_page() {
        let g = init_db("bridged_tokens_last").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        for nm in ["A", "B", "C"] {
            seed_asset_edges(db.as_ref(), Some(nm.into()), vec![(1, 2, 1)]).await;
        }

        let (rows, pag) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            2,
            true,
            None,
            None,
        )
        .await
        .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name.as_deref(), Some("B"));
        assert_eq!(rows[1].name.as_deref(), Some("C"));
        assert!(pag.next_marker.is_none());
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn bridged_tokens_token_list_and_metadata_join() {
        let g = init_db("bridged_tokens_meta").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        let aid = seed_asset_edges(db.as_ref(), Some("Tok".into()), vec![(1, 2, 1)]).await;
        let addr1 = vec![0x11u8; 20];
        let addr2 = vec![0x22u8; 20];
        stats_asset_tokens::Entity::insert(stats_asset_tokens::ActiveModel {
            stats_asset_id: Set(aid),
            chain_id: Set(1),
            token_address: Set(addr1.clone()),
            ..Default::default()
        })
        .exec(db.as_ref())
        .await
        .unwrap();
        stats_asset_tokens::Entity::insert(stats_asset_tokens::ActiveModel {
            stats_asset_id: Set(aid),
            chain_id: Set(2),
            token_address: Set(addr2.clone()),
            ..Default::default()
        })
        .exec(db.as_ref())
        .await
        .unwrap();

        tokens::Entity::insert(tokens::ActiveModel {
            chain_id: Set(1),
            address: Set(addr1.clone()),
            name: Set(Some("TName".into())),
            symbol: Set(Some("TS".into())),
            token_icon: Set(Some("http://i".into())),
            decimals: Set(Some(8)),
            ..Default::default()
        })
        .exec(db.as_ref())
        .await
        .unwrap();

        let m = fetch_bridged_token_items_for_assets(db.as_ref(), &[aid])
            .await
            .unwrap();
        let list = m.get(&aid).unwrap();
        assert_eq!(list.len(), 2);
        let enriched = list.iter().find(|t| t.chain_id == 1).unwrap();
        assert_eq!(enriched.name.as_deref(), Some("TName"));
        assert_eq!(enriched.symbol.as_deref(), Some("TS"));
        assert_eq!(enriched.icon_url.as_deref(), Some("http://i"));
        assert_eq!(enriched.decimals, Some(8));
        let bare = list.iter().find(|t| t.chain_id == 2).unwrap();
        assert!(bare.name.is_none());
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn stats_service_bridged_tokens_wiring() {
        let g = init_db("stats_svc_bridged").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        let _ = seed_asset_edges(db.as_ref(), Some("S".into()), vec![(1, 2, 2)]).await;
        let db_arc = std::sync::Arc::new(crate::InterchainDatabase::new(db.clone()));
        let stats = crate::StatsService::new(db_arc, None);
        let (rows, p) = stats
            .get_bridged_tokens_for_chain(
                1,
                BridgedTokensSortField::Name,
                StatsSortOrder::Asc,
                10,
                false,
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].tokens.is_empty());
        assert!(p.prev_marker.is_some());
        assert!(p.next_marker.is_none());
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn bridged_tokens_q_case_insensitive_on_asset_name() {
        let g = init_db("bridged_tokens_q_case").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        let _ = seed_asset_edges(
            db.as_ref(),
            Some("CamelCaseToken".into()),
            vec![(1, 2, 1)],
        )
        .await;

        let (rows, _) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            50,
            false,
            None,
            Some("camel"),
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name.as_deref(), Some("CamelCaseToken"));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn bridged_tokens_q_ignores_linked_token_name_only() {
        let g = init_db("bridged_tokens_q_not_token").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        let aid = seed_asset_edges(
            db.as_ref(),
            Some("RowName".into()),
            vec![(1, 2, 1)],
        )
        .await;
        let addr = vec![0x11u8; 20];
        stats_asset_tokens::Entity::insert(stats_asset_tokens::ActiveModel {
            stats_asset_id: Set(aid),
            chain_id: Set(1),
            token_address: Set(addr.clone()),
            ..Default::default()
        })
        .exec(db.as_ref())
        .await
        .unwrap();
        tokens::Entity::insert(tokens::ActiveModel {
            chain_id: Set(1),
            address: Set(addr),
            name: Set(Some("OnlyInTokensTable".into())),
            ..Default::default()
        })
        .exec(db.as_ref())
        .await
        .unwrap();

        let (rows, _) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            50,
            false,
            None,
            Some("OnlyInTokens"),
        )
        .await
        .unwrap();

        assert!(rows.is_empty());

        let (rows, _) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            50,
            false,
            None,
            Some("RowName"),
        )
        .await
        .unwrap();

        assert_eq!(rows.len(), 1);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn bridged_tokens_q_malicious_string_is_literal_substring() {
        let g = init_db("bridged_tokens_q_inj").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        let _ = seed_asset_edges(db.as_ref(), Some("Alpha".into()), vec![(1, 2, 1)]).await;
        let _ = seed_asset_edges(db.as_ref(), Some("Beta".into()), vec![(1, 2, 1)]).await;

        let needle = "' OR 1=1 --";
        let (rows, _) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            50,
            false,
            None,
            Some(needle),
        )
        .await
        .unwrap();

        assert!(rows.is_empty());
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn bridged_tokens_q_pagination_stable_with_same_q() {
        let g = init_db("bridged_tokens_q_page").await;
        let db = g.client();
        seed_chains(db.as_ref(), &[1, 2]).await;
        for nm in ["GrpA", "GrpB", "Zed"] {
            seed_asset_edges(db.as_ref(), Some(nm.into()), vec![(1, 2, 1)]).await;
        }

        let (p1, pag1) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            1,
            false,
            None,
            Some("Grp"),
        )
        .await
        .unwrap();
        assert_eq!(p1.len(), 1);
        assert_eq!(p1[0].name.as_deref(), Some("GrpA"));
        let next_tok = pag1.next_marker.expect("next");

        let (p2, pag2) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            1,
            false,
            Some(next_tok),
            Some("Grp"),
        )
        .await
        .unwrap();
        assert_eq!(p2[0].name.as_deref(), Some("GrpB"));
        let prev_tok = pag2.prev_marker.expect("prev");

        let (p1b, _) = list_bridged_token_stats_for_chain(
            db.as_ref(),
            1,
            BridgedTokensSortField::Name,
            StatsSortOrder::Asc,
            1,
            false,
            Some(prev_tok),
            Some("Grp"),
        )
        .await
        .unwrap();
        assert_eq!(p1b[0].name.as_deref(), Some("GrpA"));
    }
}
