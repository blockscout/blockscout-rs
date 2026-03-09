use sea_orm_migration::prelude::*;
use see_migration_test_helpers::EmptyStruct;

#[derive(DeriveMigrationName, EmptyStruct)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                UPDATE "operation" AS o
                SET "op_type" = 'INSUFFICIENT-FEE'
                WHERE o."op_type" = 'PENDING'
                    AND EXISTS (
                        SELECT 1
                        FROM "operation_stage" AS os
                        WHERE os."operation_id" = o."id"
                            AND os."success" = FALSE
                            AND os."note" ILIKE '%insufficient%'
                            AND os."note" ILIKE '%fee%'
                    )
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                UPDATE "operation"
                SET "op_type" = 'PENDING'
                WHERE "op_type" = 'INSUFFICIENT-FEE'
                "#,
            )
            .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blockscout_service_launcher::test_database::TestDbGuard;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Statement};
    use see_migration_test_helpers::{MigratorBeforeTested, MigratorWithTested};

    type MigratorBefore = MigratorBeforeTested<crate::Migrator, super::Migration>;
    type MigratorAfter = MigratorWithTested<crate::Migrator, super::Migration>;

    #[async_std::test]
    #[ignore = "needs database to run"]
    async fn migration_marks_and_reverts_insufficient_fee_operations() {
        let db = TestDbGuard::new::<MigratorBefore>("migration_insufficient_fee").await;
        let conn = db.client();

        // op1: should be changed to INSUFFICIENT-FEE
        // op2: should be left as PENDING (op stage is successful)
        // op3: should be left as PENDING (error message doesn't match pattern)
        // op4: should be left as ROLLBACK (op_type is not PENDING)
        // op5: should be left as PENDING (op stage doesn't contain note)
        db.execute_unprepared(
            r#"
            INSERT INTO "stage_type" ("id", "name") VALUES (1, 'mock-stage');

            INSERT INTO "operation" (
                "id", "op_type", "timestamp", "next_retry", "status", "retry_count", "inserted_at", "updated_at"
            ) VALUES
                ('op1', 'PENDING',  NOW(), NULL, 'pending'::status_enum, 0, NOW(), NOW()),
                ('op2', 'PENDING',  NOW(), NULL, 'pending'::status_enum, 0, NOW(), NOW()),
                ('op3', 'PENDING',  NOW(), NULL, 'pending'::status_enum, 0, NOW(), NOW()),
                ('op4', 'ROLLBACK', NOW(), NULL, 'pending'::status_enum, 0, NOW(), NOW()),
                ('op5', 'PENDING',  NOW(), NULL, 'pending'::status_enum, 0, NOW(), NOW());

            INSERT INTO "operation_stage" (
                "operation_id", "stage_type_id", "success", "timestamp", "note", "inserted_at"
            ) VALUES
                ('op1', 1, FALSE, NOW(), '{"content":"insufficient executor fee","errorName":"","internalBytesError":"","internalMsg":""}', NOW()),
                ('op2', 1, TRUE,  NOW(), 'Error: insufficient fee', NOW()),
                ('op3', 1, FALSE, NOW(), 'Error: insufficient balance', NOW()),
                ('op4', 1, FALSE, NOW(), '{"content":"insufficient extra fee"}', NOW()),
                ('op5', 1, FALSE, NOW(), NULL, NOW());
            "#,
        )
        .await
        .expect("failed to prepare test data");

        MigratorAfter::up(conn.as_ref(), None)
            .await
            .expect("failed to apply target migration");

        assert_eq!(
            fetch_op_type(conn.as_ref(), "op1").await,
            Some("INSUFFICIENT-FEE".to_string())
        );
        assert_eq!(
            fetch_op_type(conn.as_ref(), "op2").await,
            Some("PENDING".to_string())
        );
        assert_eq!(
            fetch_op_type(conn.as_ref(), "op3").await,
            Some("PENDING".to_string())
        );
        assert_eq!(
            fetch_op_type(conn.as_ref(), "op4").await,
            Some("ROLLBACK".to_string())
        );
        assert_eq!(
            fetch_op_type(conn.as_ref(), "op5").await,
            Some("PENDING".to_string())
        );

        MigratorAfter::down(conn.as_ref(), Some(1))
            .await
            .expect("failed to roll back target migration");

        assert_eq!(
            fetch_op_type(conn.as_ref(), "op1").await,
            Some("PENDING".to_string())
        );
        assert_eq!(
            fetch_op_type(conn.as_ref(), "op2").await,
            Some("PENDING".to_string())
        );
        assert_eq!(
            fetch_op_type(conn.as_ref(), "op3").await,
            Some("PENDING".to_string())
        );
        assert_eq!(
            fetch_op_type(conn.as_ref(), "op4").await,
            Some("ROLLBACK".to_string())
        );
        assert_eq!(
            fetch_op_type(conn.as_ref(), "op5").await,
            Some("PENDING".to_string())
        );
    }

    async fn fetch_op_type(conn: &impl ConnectionTrait, id: &str) -> Option<String> {
        let row = conn
            .query_one(Statement::from_sql_and_values(
                sea_orm_migration::sea_orm::DatabaseBackend::Postgres,
                r#"SELECT "op_type" FROM "operation" WHERE "id" = $1"#,
                vec![id.into()],
            ))
            .await
            .expect("failed to fetch operation type");

        row.and_then(|row| row.try_get("", "op_type").ok())
    }
}
