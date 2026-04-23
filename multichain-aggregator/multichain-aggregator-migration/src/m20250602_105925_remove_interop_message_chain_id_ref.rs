use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE interop_messages DROP CONSTRAINT interop_messages_init_chain_id_fkey;
            ALTER TABLE interop_messages DROP CONSTRAINT interop_messages_relay_chain_id_fkey;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE interop_messages ADD CONSTRAINT interop_messages_init_chain_id_fkey FOREIGN KEY (init_chain_id) REFERENCES chains (id);
            ALTER TABLE interop_messages ADD CONSTRAINT interop_messages_relay_chain_id_fkey FOREIGN KEY (relay_chain_id) REFERENCES chains (id);
        "#;
        crate::from_sql(manager, sql).await
    }
}
