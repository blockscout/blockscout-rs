// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{
    entity::subgraph::domain::DomainWithAddress,
    protocols::Protocol,
    subgraph::sql::{utils, DbErr, DOMAIN_BLOCK_RANGE_WHERE_CLAUSE},
};
use nonempty::NonEmpty;
use sea_query::{Alias, Expr, PostgresQueryBuilder};
use sqlx::PgPool;

pub struct PrimaryNameRecordTable;

impl PrimaryNameRecordTable {
    fn table_name() -> &'static str {
        "primary_name_record"
    }

    // Some protocols require current ownership and normal (not grace-extended)
    // expiry for a primary record. An indexed record can outlive expiry without
    // an event at that boundary.
    fn active_owner_guard(schema: &str) -> String {
        let schema = schema.replace('"', "\"\"");
        format!(
            "EXISTS (SELECT 1 FROM \"{schema}\".\"domain\" AS d \
             WHERE d.id = primary_name_record.domain_id \
             AND d.name = primary_name_record.domain_name \
             AND d.owner = primary_name_record.resolved_address \
             AND d.block_range @> 2147483647 \
             AND d.expiry_date IS NOT NULL \
             AND d.expiry_date > extract(epoch from now()))"
        )
    }

    fn query_for_protocol(protocol: &Protocol) -> sea_query::SelectStatement {
        let mut query = sea_query::Query::select();
        query
            .expr(Expr::cust("domain_id as id"))
            .expr(Expr::cust("domain_name"))
            .expr(Expr::cust("resolved_address"))
            .expr(Expr::cust(format!(
                "'{}' as protocol_slug",
                protocol.info.slug
            )))
            .from((
                Alias::new(&protocol.subgraph_schema),
                Alias::new(Self::table_name()),
            ))
            .and_where(Expr::cust("resolved_address = ANY($1)"))
            .and_where(Expr::cust("domain_id is not null"))
            .and_where(Expr::cust("domain_name is not null"))
            .and_where(Expr::cust(DOMAIN_BLOCK_RANGE_WHERE_CLAUSE));

        if protocol.info.primary_name_record_requires_active_owner {
            query.and_where(Expr::cust(Self::active_owner_guard(
                &protocol.subgraph_schema,
            )));
        }
        query
    }
}

impl PrimaryNameRecordTable {
    pub async fn batch_search_addresses(
        pool: &PgPool,
        protocols: &NonEmpty<&Protocol>,
        address: &[impl AsRef<str>],
    ) -> Result<Vec<DomainWithAddress>, DbErr> {
        let queries = NonEmpty::collect(protocols.into_iter().map(|p| Self::query_for_protocol(p)))
            .expect("protocols is nonempty");
        let sql = utils::union_domain_queries(queries, None, None)?.to_string(PostgresQueryBuilder);
        let domains = sqlx::query_as(&sql)
            .bind(utils::bind_string_list(address))
            .fetch_all(pool)
            .await?;
        Ok(domains)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::insert_rensa_fixture_domain;
    use nonempty::nonempty;
    use sqlx::PgPool;

    #[test]
    fn rensa_reverse_requires_current_owner_and_active_expiry() {
        let mut protocol = Protocol::default();
        protocol.info.slug = "rensa".into();
        protocol.info.primary_name_record_requires_active_owner = true;
        protocol.subgraph_schema = "sgd_rensa".into();
        let sql =
            PrimaryNameRecordTable::query_for_protocol(&protocol).to_string(PostgresQueryBuilder);

        assert!(sql.contains("d.id = primary_name_record.domain_id"));
        assert!(sql.contains("d.name = primary_name_record.domain_name"));
        assert!(sql.contains("d.owner = primary_name_record.resolved_address"));
        assert!(sql.contains("d.block_range @> 2147483647"));
        assert!(sql.contains("d.expiry_date IS NOT NULL"));
        assert!(sql.contains("d.expiry_date > extract(epoch from now())"));
    }

    #[test]
    fn existing_primary_name_protocol_queries_are_unchanged() {
        let mut protocol = Protocol::default();
        protocol.info.slug = "infinityname-base".into();
        protocol.subgraph_schema = "sgd_infinity".into();
        let sql =
            PrimaryNameRecordTable::query_for_protocol(&protocol).to_string(PostgresQueryBuilder);

        assert!(!sql.contains("EXISTS (SELECT 1"));
        assert!(sql.contains("domain_id is not null"));
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn guarded_primary_record_stops_at_expiry_and_owner_change(pool: PgPool) {
        // A .rns domain is inserted into Graph-node's real domain table shape.
        // The primary table is added because the ENS fixture predates it.
        sqlx::query(
            "CREATE TABLE sgd1.primary_name_record (block_range int4range NOT NULL, \
             resolved_address text NOT NULL, domain_id text, domain_name text)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let owner = "0xd8da6bf26964af9d7eed9e03e53415d37aa96045";
        let domain_id = insert_rensa_fixture_domain(&pool, owner).await;
        sqlx::query(
            "INSERT INTO sgd1.primary_name_record \
             (block_range, resolved_address, domain_id, domain_name) \
             VALUES ('[1,)', $1, $2, 'rensa.rns')",
        )
        .bind(owner)
        .bind(&domain_id)
        .execute(&pool)
        .await
        .unwrap();
        let mut protocol = Protocol::default();
        protocol.info.slug = "rensa".into();
        protocol.info.forward_resolution_grace_period_seconds = 7_776_000;
        protocol.info.primary_name_record_requires_active_owner = true;
        protocol.subgraph_schema = "sgd1".into();

        for (seconds_from_now, expected_reverse) in [
            (600_i64, true),
            (-600, false), // Forward resolution remains valid here, but reverse does not.
            (-7_776_001, false),
        ] {
            sqlx::query(
                "UPDATE sgd1.domain SET expiry_date = floor(extract(epoch from now())) + $1 \
                 WHERE id = $2 AND block_range @> 2147483647",
            )
            .bind(seconds_from_now)
            .bind(&domain_id)
            .execute(&pool)
            .await
            .unwrap();
            let records = PrimaryNameRecordTable::batch_search_addresses(
                &pool,
                &nonempty![&protocol],
                &[owner],
            )
            .await
            .unwrap();
            assert_eq!(!records.is_empty(), expected_reverse);
        }

        // A transfer changes current ownership, invalidating the old primary
        // even if the indexed primary event has not yet been removed.
        let new_owner = "0x1111111111111111111111111111111111111111";
        sqlx::query(
            "UPDATE sgd1.domain SET expiry_date = floor(extract(epoch from now())) + 600, \
             owner = $2 \
             WHERE id = $1 AND block_range @> 2147483647",
        )
        .bind(&domain_id)
        .bind(new_owner)
        .execute(&pool)
        .await
        .unwrap();
        assert!(PrimaryNameRecordTable::batch_search_addresses(
            &pool,
            &nonempty![&protocol],
            &[owner],
        )
        .await
        .unwrap()
        .is_empty());
        assert!(PrimaryNameRecordTable::batch_search_addresses(
            &pool,
            &nonempty![&protocol],
            &[new_owner],
        )
        .await
        .unwrap()
        .is_empty());

        // Finalization/burn must not revive a stale primary record either.
        sqlx::query(
            "UPDATE sgd1.domain SET owner = '0x0000000000000000000000000000000000000000' \
             WHERE id = $1 AND block_range @> 2147483647",
        )
        .bind(&domain_id)
        .execute(&pool)
        .await
        .unwrap();
        assert!(PrimaryNameRecordTable::batch_search_addresses(
            &pool,
            &nonempty![&protocol],
            &[owner],
        )
        .await
        .unwrap()
        .is_empty());

        // The opt-in flag's default leaves legacy primary-name queries alone.
        protocol.info.primary_name_record_requires_active_owner = false;
        assert_eq!(
            PrimaryNameRecordTable::batch_search_addresses(&pool, &nonempty![&protocol], &[owner])
                .await
                .unwrap()
                .len(),
            1
        );
        sqlx::query("DELETE FROM sgd1.primary_name_record WHERE resolved_address = $1")
            .bind(owner)
            .execute(&pool)
            .await
            .unwrap();
        assert!(PrimaryNameRecordTable::batch_search_addresses(
            &pool,
            &nonempty![&protocol],
            &[owner],
        )
        .await
        .unwrap()
        .is_empty());
    }
}
