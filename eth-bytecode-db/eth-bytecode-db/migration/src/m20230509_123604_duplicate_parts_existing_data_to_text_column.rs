use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let batch_size = 1000;

        // We update existing rows in batches, so not to load the database too much.
        // In case if any error occurs during execution, already processed batches
        // will not be rolled back, but nothing bad happens.
        let sql = format!(
            r#"
            UPDATE "parts"
            SET "data_text" = encode("data", 'hex')
            WHERE "id" IN (SELECT "id"
                           FROM "parts"
                           WHERE "data_text" IS NULL
                           LIMIT {batch_size});
        "#
        );

        let mut rows_affected = batch_size;
        while rows_affected >= batch_size {
            rows_affected = manager
                .get_connection()
                .execute_unprepared(&sql)
                .await?
                .rows_affected();
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
