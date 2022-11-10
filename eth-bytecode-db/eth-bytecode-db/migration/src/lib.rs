pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, RuntimeErr, Statement};

mod m20220101_000001_initial;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20220101_000001_initial::Migration)]
    }
}

pub async fn from_sql(manager: &SchemaManager<'_>, content: &str) -> Result<(), DbErr> {
    let stmnts: Vec<&str> = content.split(';').collect();
    for st in stmnts.into_iter() {
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                st.to_string(),
            ))
            .await
            .map_err(|_| DbErr::Query(RuntimeErr::Internal(st.to_string())))?;
    }
    Ok(())
}
