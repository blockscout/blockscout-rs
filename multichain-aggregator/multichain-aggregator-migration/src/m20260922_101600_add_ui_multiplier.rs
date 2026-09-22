// SPDX-License-Identifier: LicenseRef-Blockscout

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE tokens ADD COLUMN IF NOT EXISTS ui_multiplier numeric(78, 0);
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE tokens DROP COLUMN IF EXISTS ui_multiplier;
        "#;
        crate::from_sql(manager, sql).await
    }
}
