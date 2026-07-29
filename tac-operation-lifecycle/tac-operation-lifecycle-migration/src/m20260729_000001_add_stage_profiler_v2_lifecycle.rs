// SPDX-License-Identifier: LicenseRef-Blockscout

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
                ALTER TABLE "operation"
                    ADD COLUMN "profiling_version" SMALLINT NOT NULL DEFAULT 1,
                    ADD COLUMN "op_status" TEXT NULL,
                    ADD COLUMN "finalized" BOOLEAN NULL,
                    ADD COLUMN "rollback" BOOLEAN NULL;

                CREATE INDEX "idx_operation_v2_pending"
                    ON "operation" ("timestamp" DESC)
                    WHERE "status" = 'pending'::status_enum
                      AND "profiling_version" = 2
                      AND "finalized" = FALSE;

                CREATE INDEX "idx_operation_v1_backfill"
                    ON "operation" ("timestamp" DESC)
                    WHERE "op_type" IS NOT NULL
                      AND "profiling_version" = 1
                      AND "status" IN ('pending'::status_enum, 'completed'::status_enum);

                CREATE INDEX "idx_operation_stage_operation_id"
                    ON "operation_stage" ("operation_id");
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
                UPDATE "operation" AS o
                SET "op_type" = CASE
                    WHEN o."finalized" = FALSE
                         AND o."op_status" = 'failed'
                         AND EXISTS (
                            SELECT 1 FROM "operation_stage" AS os
                            WHERE os."operation_id" = o."id"
                              AND os."success" = FALSE
                              AND os."note" ILIKE '%insufficient%'
                              AND os."note" ILIKE '%fee%'
                         )
                        THEN 'INSUFFICIENT-FEE'
                    WHEN o."finalized" = FALSE THEN 'PENDING'
                    WHEN o."finalized" = TRUE
                         AND o."op_status" = 'failed'
                         AND o."rollback" = TRUE
                        THEN 'ROLLBACK'
                    WHEN o."op_type" IN ('TON-TAC-TON', 'TAC-TON', 'TON-TAC', 'UNKNOWN')
                        THEN o."op_type"
                    ELSE 'ERROR'
                END
                WHERE o."profiling_version" = 2;

                DROP INDEX "idx_operation_stage_operation_id";
                DROP INDEX "idx_operation_v1_backfill";
                DROP INDEX "idx_operation_v2_pending";

                ALTER TABLE "operation"
                    DROP COLUMN "rollback",
                    DROP COLUMN "finalized",
                    DROP COLUMN "op_status",
                    DROP COLUMN "profiling_version";
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
    async fn preserves_legacy_rows_and_projects_v2_on_down() {
        let db = TestDbGuard::new::<MigratorBefore>("migration_stage_profiler_v2").await;
        let conn = db.client();
        conn.execute_unprepared(
            r#"
            INSERT INTO "stage_type" ("id", "name") VALUES (1, 'mock-stage');
            INSERT INTO "operation"
                ("id", "op_type", "timestamp", "next_retry", "status", "retry_count", "inserted_at", "updated_at")
            VALUES
                ('route', 'TON-TAC-TON', NOW(), NULL, 'completed', 0, NOW(), NOW()),
                ('pending', 'PENDING', NOW(), NULL, 'pending', 0, NOW(), NOW()),
                ('insufficient', 'INSUFFICIENT-FEE', NOW(), NULL, 'pending', 0, NOW(), NOW()),
                ('rollback-op', 'ROLLBACK', NOW(), NULL, 'completed', 0, NOW(), NOW()),
                ('unknown', 'UNKNOWN', NOW(), NULL, 'pending', 0, NOW(), NOW()),
                ('error', 'ERROR', NOW(), NULL, 'completed', 0, NOW(), NOW()),
                ('forever', 'PENDING', NOW(), NULL, 'completed', 0, NOW(), NOW());
            "#,
        )
        .await
        .unwrap();

        MigratorAfter::up(conn.as_ref(), None).await.unwrap();
        for (id, expected_type, expected_status) in [
            ("route", "TON-TAC-TON", "completed"),
            ("pending", "PENDING", "pending"),
            ("insufficient", "INSUFFICIENT-FEE", "pending"),
            ("rollback-op", "ROLLBACK", "completed"),
            ("unknown", "UNKNOWN", "pending"),
            ("error", "ERROR", "completed"),
            ("forever", "PENDING", "completed"),
        ] {
            let row = conn
                .query_one(Statement::from_sql_and_values(
                    sea_orm_migration::sea_orm::DatabaseBackend::Postgres,
                    "SELECT op_type, profiling_version, status::text status FROM operation WHERE id=$1",
                    [id.into()],
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.try_get::<String>("", "op_type").unwrap(), expected_type);
            assert_eq!(row.try_get::<i16>("", "profiling_version").unwrap(), 1);
            assert_eq!(
                row.try_get::<String>("", "status").unwrap(),
                expected_status
            );
        }

        conn.execute_unprepared(
            r#"
            UPDATE operation SET profiling_version=2, op_type='TON-TAC-TON',
                op_status='success', finalized=TRUE, rollback=FALSE WHERE id='route';
            UPDATE operation SET profiling_version=2, op_type='TON-TAC',
                op_status='success', finalized=FALSE, rollback=FALSE WHERE id='pending';
            UPDATE operation SET profiling_version=2, op_type='TON-TAC',
                op_status='failed', finalized=FALSE, rollback=TRUE WHERE id='insufficient';
            UPDATE operation SET profiling_version=2, op_type='TAC-TON',
                op_status='failed', finalized=TRUE, rollback=TRUE WHERE id='rollback-op';
            UPDATE operation SET profiling_version=2, op_type='UNKNOWN',
                op_status=NULL, finalized=TRUE, rollback=FALSE WHERE id='unknown';
            UPDATE operation SET profiling_version=2, op_type='FUTURE-ROUTE',
                op_status='success', finalized=TRUE, rollback=FALSE WHERE id='error';
            UPDATE operation SET profiling_version=2, op_type='TON-TAC',
                op_status='failed', finalized=FALSE, rollback=TRUE WHERE id='forever';
            INSERT INTO operation_stage
                (operation_id, stage_type_id, success, timestamp, note, inserted_at)
            VALUES
                ('insufficient', 1, FALSE, NOW(), 'Insufficient executor fee', NOW()),
                ('forever', 1, FALSE, NOW(), 'Insufficient executor fee', NOW());
            "#,
        )
        .await
        .unwrap();
        MigratorAfter::down(conn.as_ref(), Some(1)).await.unwrap();
        for (id, expected_type, expected_status) in [
            ("route", "TON-TAC-TON", "completed"),
            ("pending", "PENDING", "pending"),
            ("insufficient", "INSUFFICIENT-FEE", "pending"),
            ("rollback-op", "ROLLBACK", "completed"),
            ("unknown", "UNKNOWN", "pending"),
            ("error", "ERROR", "completed"),
            ("forever", "INSUFFICIENT-FEE", "completed"),
        ] {
            let row = conn
                .query_one(Statement::from_sql_and_values(
                    sea_orm_migration::sea_orm::DatabaseBackend::Postgres,
                    "SELECT op_type, status::text status FROM operation WHERE id=$1",
                    [id.into()],
                ))
                .await
                .unwrap()
                .unwrap();
            assert_eq!(row.try_get::<String>("", "op_type").unwrap(), expected_type);
            assert_eq!(
                row.try_get::<String>("", "status").unwrap(),
                expected_status
            );
        }
    }
}
