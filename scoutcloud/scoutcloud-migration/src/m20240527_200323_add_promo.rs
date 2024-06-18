use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            r#"
            CREATE TABLE "register_promo" (
                "id" SERIAL PRIMARY KEY,
                "name" TEXT NOT NULL,
                "code" TEXT NOT NULL,
                "user_initial_balance" DECIMAL NOT NULL,
                "user_max_instances" INTEGER NOT NULL,
                "max_activations" INTEGER NOT NULL,
                "deleted" BOOL NOT NULL DEFAULT FALSE,
                "created_at" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
                "updated_at" TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
            );

            ALTER TABLE "balance_changes" ADD COLUMN register_promo_id INTEGER REFERENCES "register_promo" ("id") ON DELETE SET NULL;
            "#,
        )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            r#"
            ALTER TABLE "balance_changes" DROP COLUMN "register_promo_id";
            DROP TABLE "register_promo";
            "#,
        )
        .await
    }
}
