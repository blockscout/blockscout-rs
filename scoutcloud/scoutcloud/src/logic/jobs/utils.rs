macro_rules! impl_get_db {
    ($struct:ident) => {
        impl $struct {
            async fn get_db(&self) -> std::sync::Arc<sea_orm::DatabaseConnection> {
                #[cfg(test)]
                {
                    let database_url = self
                        .database_url
                        .clone()
                        .expect("database_url is not set in tests");
                    let conn = sea_orm::Database::connect(database_url)
                        .await
                        .expect("cannot connect to db in tests");
                    return std::sync::Arc::new(conn);
                }
                #[allow(unreachable_code)]
                crate::logic::jobs::global::get_db_connection()
            }
        }
    };
}

pub(crate) use impl_get_db;
