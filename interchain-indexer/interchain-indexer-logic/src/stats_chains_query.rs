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

pub async fn list_stats_chains(
    db: &impl ConnectionTrait,
    chain_ids: &[i64],
    include_zero_chains: bool,
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

    if !chain_ids.is_empty() {
        let placeholders: Vec<String> = (0..chain_ids.len())
            .map(|i| format!("${}", i + 1))
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
        inner_conditions.push(
            "(COALESCE(sc.unique_transfer_users_count, 0) > 0 OR COALESCE(sc.unique_message_users_count, 0) > 0)"
                .to_string(),
        );
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
           COALESCE(sc.unique_transfer_users_count, 0)::bigint AS cnt
    FROM chains c
    LEFT JOIN stats_chains sc ON sc.chain_id = c.id
    {inner_where}
) t
WHERE TRUE
{where_extra}
ORDER BY {order_clause}
LIMIT ${limit_ph}
"#,
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
    use sea_orm::{ActiveValue::Set, DatabaseConnection, EntityTrait};

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
            &[],
            true,
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
            &[],
            false,
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
            &[],
            false,
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
            &[],
            false,
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
            &[],
            true,
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
            &[],
            true,
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
            &[],
            true,
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
            &[],
            true,
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
            &[],
            true,
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
            &[3, 1],
            true,
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
            &[],
            true,
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
            &[],
            true,
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
            &[],
            true,
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
            &[],
            true,
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
            &[],
            true,
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
            &[],
            true,
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
}
