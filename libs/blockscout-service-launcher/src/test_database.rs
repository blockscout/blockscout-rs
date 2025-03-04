/// Postgres supports maximum 63 symbols.
/// All exceeding symbols are truncated by the database.
const MAX_DATABASE_NAME_LEN: usize = 63;

/// A length of the hex encoded hash of database name
/// when the original exceeds [`MAX_DATABASE_NAME_LEN`]
const HASH_SUFFIX_STRING_LEN: usize = 8;

/// Generates a unique database name for use in tests.
///
/// This macro creates a database name based on the file name, line number, and column number
/// of the macro invocation. Optionally, a custom prefix can be appended for added specificity,
/// which is useful in scenarios like parameterized tests.
///
/// For more details on usage and examples, see the [`database!`](macro.database.html) macro.
///
/// # Arguments
///
/// - `custom_prefix` (optional): A custom string to append to the database name.
#[macro_export]
macro_rules! database_name {
    () => {
        format!("{}_{}_{}", file!(), line!(), column!())
    };
    ($custom_prefix:expr) => {
        format!("{}_{}_{}_{}", $custom_prefix, file!(), line!(), column!())
    };
}
pub use database_name;

/// Initializes a test database for use in tests.
///
/// This macro simplifies setting up a database by automatically generating a database name
/// based on the location where the function is defined. It eliminates the need to manually
/// specify the test case name for the database name.
///
/// # Usage
///
/// The macro can be used within a test as follows:
/// ```text
/// use blockscout_service_launcher::test_database::database;
///
/// #[tokio::test]
/// async fn test() {
///     let db_guard = database!(Migrator);
///     // Perform operations with the database...
/// }
/// ```
///
/// The `Migrator` parameter refers to the struct implementing
/// `sea_orm_migration::MigratorTrait` associated with the database.
///
/// # Parameterized Tests
///
/// **Note:** When using this macro with [`rstest` parameterized test cases](https://docs.rs/rstest/latest/rstest/attr.rstest.html#test-parametrized-cases),
/// the same database name will be used for all test cases. To avoid conflicts, you need to provide
/// a meaningful prefix explicitly, as demonstrated below:
///
/// ```text
/// #[tokio::test]
/// async fn test_with_prefix() {
///     let db_guard = database!(Migrator, "custom_prefix");
///     // Perform operations with the database...
/// }
/// ```
#[macro_export]
macro_rules! database {
    ($migrator:ty) => {{
        $crate::test_database::TestDbGuard::new::<$migrator>(
            &$crate::test_database::database_name!(),
        )
        .await
    }};
    ($migrator:ty, $custom_prefix:expr) => {{
        $crate::test_database::TestDbGuard::new::<$migrator>(
            &$crate::test_database::database_name!($custom_prefix),
        )
        .await
    }};
}
pub use database;

cfg_if::cfg_if! {
    if #[cfg(feature = "database-1")] {
        pub use db_guard::async_dropped::TestDbGuardAsyncDropped as TestDbGuard;
        pub use db_guard::TestDbGuardInner;
    } else {
        pub use db_guard::TestDbGuardInner as TestDbGuard;
    }
}

mod db_guard {
    use crate::{
        database::{
            ConnectionTrait, Database, DatabaseConnection, DbErr, MigratorTrait, Statement,
        },
        test_database::{HASH_SUFFIX_STRING_LEN, MAX_DATABASE_NAME_LEN},
    };
    use std::{ops::Deref, sync::Arc};

    #[derive(Clone, Debug)]
    pub struct TestDbGuardInner {
        conn_with_db: Arc<DatabaseConnection>,
        conn_without_db: Arc<DatabaseConnection>,
        base_db_url: String,
        db_name: String,
    }

    impl Default for TestDbGuardInner {
        fn default() -> Self {
            Self {
                conn_with_db: Arc::new(DatabaseConnection::Disconnected),
                conn_without_db: Arc::new(DatabaseConnection::Disconnected),
                base_db_url: Default::default(),
                db_name: Default::default(),
            }
        }
    }

    impl TestDbGuardInner {
        pub async fn new<Migrator: MigratorTrait>(db_name: &str) -> Self {
            let base_db_url = std::env::var("DATABASE_URL")
                .expect("Database url must be set to initialize a test database")
                .trim_end_matches('/')
                .to_string();
            let conn_without_db = Database::connect(&base_db_url)
                .await
                .expect("Connection to postgres (without database) failed");
            let db_name = Self::preprocess_database_name(db_name);
            let mut guard = TestDbGuardInner {
                conn_with_db: Arc::new(DatabaseConnection::Disconnected),
                conn_without_db: Arc::new(conn_without_db),
                base_db_url,
                db_name,
            };

            guard.init_database().await;
            guard.run_migrations::<Migrator>().await;
            guard
        }

        /// Creates a new test database helper with a unique name.
        ///
        /// This function initializes a test database, where the database name is constructed
        /// as a concatenation of the provided `prefix_name`, `file`, `line`, and `column` arguments.
        /// It ensures that the generated database name is unique to the location in the code
        /// where this function is called.
        ///
        /// # Arguments
        ///
        /// - `prefix_name`: A custom prefix for the database name.
        /// - `file`: The file name where this function is invoked. Must be the result of the `file!` macro.
        /// - `line`: The line number where this function is invoked. Must be the result of the `line!` macro.
        /// - `column`: The column number where this function is invoked. Must be the result of the `column!` macro.
        ///
        /// # Example
        ///
        /// ```text
        /// let db_guard = TestDbGuard::new_with_metadata::<Migrator>("test_db", file!(), line!(), column!()).await;
        /// ```
        pub async fn new_with_metadata<Migrator: MigratorTrait>(
            prefix_name: &str,
            file: &str,
            line: u32,
            column: u32,
        ) -> Self {
            let db_name = format!("{prefix_name}_{file}_{line}_{column}");
            Self::new::<Migrator>(db_name.as_str()).await
        }

        pub fn client(&self) -> Arc<DatabaseConnection> {
            self.conn_with_db.clone()
        }

        pub fn db_url(&self) -> String {
            format!("{}/{}", self.base_db_url, self.db_name)
        }

        async fn init_database(&mut self) {
            // Create database
            self.drop_database().await;
            self.create_database().await;

            let db_url = self.db_url();
            let conn_with_db = Database::connect(&db_url)
                .await
                .expect("Connection to postgres (with database) failed");
            self.conn_with_db = Arc::new(conn_with_db);
        }

        pub async fn drop_database(&self) {
            Self::drop_database_internal(&self.conn_without_db, &self.db_name)
                .await
                .expect("Database drop failed");
        }

        async fn create_database(&self) {
            Self::create_database_internal(&self.conn_without_db, &self.db_name)
                .await
                .expect("Database creation failed");
        }

        async fn create_database_internal(
            db: &DatabaseConnection,
            db_name: &str,
        ) -> Result<(), DbErr> {
            tracing::info!(name = db_name, "creating database");
            db.execute(Statement::from_string(
                db.get_database_backend(),
                format!("CREATE DATABASE \"{db_name}\""),
            ))
            .await?;
            Ok(())
        }

        async fn drop_database_internal(
            db: &DatabaseConnection,
            db_name: &str,
        ) -> Result<(), DbErr> {
            tracing::info!(name = db_name, "dropping database");
            db.execute(Statement::from_string(
                db.get_database_backend(),
                format!("DROP DATABASE IF EXISTS \"{db_name}\" WITH (FORCE)"),
            ))
            .await?;
            Ok(())
        }

        async fn run_migrations<Migrator: MigratorTrait>(&self) {
            Migrator::up(self.conn_with_db.as_ref(), None)
                .await
                .expect("Database migration failed");
        }

        /// Strips given database name if the one is too long to be supported.
        /// To differentiate the resultant name with other similar prefixes,
        /// a 4-bytes hash of the original name is added at the end.
        fn preprocess_database_name(name: &str) -> String {
            if name.len() <= MAX_DATABASE_NAME_LEN {
                return name.to_string();
            }

            let hash = &format!("{:x}", keccak_hash::keccak(name))[..HASH_SUFFIX_STRING_LEN];
            format!(
                "{}-{hash}",
                &name[..MAX_DATABASE_NAME_LEN - HASH_SUFFIX_STRING_LEN - 1]
            )
        }

        pub async fn close_all(&self) -> (Result<(), DbErr>, Result<(), DbErr>) {
            // remove `get_postgres_connection_pool` as soon as
            // https://github.com/SeaQL/sea-orm/pull/2511 is available
            // to work with other connections
            self.conn_with_db
                .get_postgres_connection_pool()
                .close()
                .await;
            self.conn_without_db
                .get_postgres_connection_pool()
                .close()
                .await;
            (Ok(()), Ok(()))
        }

        pub async fn close_all_unwrap(self) {
            let (res_with_db, res_without_db) = self.close_all().await;
            res_with_db.expect("couldn't close connection with database");
            res_without_db.expect("couldn't close connection without database");
        }
    }

    impl Deref for TestDbGuardInner {
        type Target = DatabaseConnection;
        fn deref(&self) -> &Self::Target {
            &self.conn_with_db
        }
    }

    #[cfg(feature = "database-1")]
    pub(crate) mod async_dropped {
        use std::{ops::Deref, sync::Arc};

        use async_dropper::{AsyncDrop, AsyncDropper};
        use async_trait::async_trait;

        use crate::database::{DatabaseConnection, MigratorTrait};

        use super::TestDbGuardInner;

        #[async_trait]
        impl AsyncDrop for TestDbGuardInner {
            async fn async_drop(&mut self) {
                tracing::info!(
                    db_url = self.db_url(),
                    "Closing connections on test db guard"
                );
                if let Err(e) = self.conn_with_db.close_by_ref().await {
                    tracing::warn!(db_url = self.db_url(), "Could not close connection: {}", e);
                }
                if let Err(e) = self.conn_without_db.close_by_ref().await {
                    tracing::warn!(
                        db_url = self.db_url(),
                        "Could not close connection without db ({}): {}",
                        self.base_db_url,
                        e
                    );
                }
                tracing::info!(
                    db_url = self.db_url(),
                    "Finished closing connections on test db guard"
                );
            }
        }

        /// Wrapper over `TestDbGuardInner` to allow closing
        /// connection when the guard is dropped. This is quite important
        /// for tests because they can exhaust postgres with dangling connecitons.
        ///
        /// Cloneing is not implemented on purpose to not call `close` unnecessary
        /// (and cause warn logs). Also `AsyncDropper` doesn't implement `Clone`.
        pub struct TestDbGuardAsyncDropped {
            pub inner: AsyncDropper<TestDbGuardInner>,
        }

        impl std::fmt::Debug for TestDbGuardAsyncDropped {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.debug_struct("TestDbGuardAsyncDropped")
                    .field("inner", &self.inner.inner())
                    .finish()
            }
        }

        impl Default for TestDbGuardAsyncDropped {
            fn default() -> Self {
                Self {
                    inner: Default::default(),
                }
            }
        }

        // implementation delegated to the inner type
        impl TestDbGuardAsyncDropped {
            pub async fn new<Migrator: MigratorTrait>(db_name: &str) -> Self {
                Self {
                    inner: AsyncDropper::new(TestDbGuardInner::new::<Migrator>(db_name).await),
                }
            }

            /// Creates a new test database helper with a unique name.
            ///
            /// This function initializes a test database, where the database name is constructed
            /// as a concatenation of the provided `prefix_name`, `file`, `line`, and `column` arguments.
            /// It ensures that the generated database name is unique to the location in the code
            /// where this function is called.
            ///
            /// # Arguments
            ///
            /// - `prefix_name`: A custom prefix for the database name.
            /// - `file`: The file name where this function is invoked. Must be the result of the `file!` macro.
            /// - `line`: The line number where this function is invoked. Must be the result of the `line!` macro.
            /// - `column`: The column number where this function is invoked. Must be the result of the `column!` macro.
            ///
            /// # Example
            ///
            /// ```text
            /// let db_guard = TestDbGuard::new_with_metadata::<Migrator>("test_db", file!(), line!(), column!()).await;
            /// ```
            pub async fn new_with_metadata<Migrator: MigratorTrait>(
                prefix_name: &str,
                file: &str,
                line: u32,
                column: u32,
            ) -> Self {
                Self {
                    inner: AsyncDropper::new(
                        TestDbGuardInner::new_with_metadata::<Migrator>(
                            prefix_name,
                            file,
                            line,
                            column,
                        )
                        .await,
                    ),
                }
            }

            pub fn client(&self) -> Arc<DatabaseConnection> {
                self.inner.client()
            }

            pub fn db_url(&self) -> String {
                self.inner.db_url()
            }

            pub async fn drop_database(&self) {
                self.inner.drop_database().await
            }
        }

        impl Deref for TestDbGuardAsyncDropped {
            type Target = DatabaseConnection;
            fn deref(&self) -> &Self::Target {
                &self.inner.deref()
            }
        }
    }
}
