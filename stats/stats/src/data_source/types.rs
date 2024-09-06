use blockscout_db::entity::migrations_status;
use chrono::Utc;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QueryOrder, Statement};
use tracing::warn;

#[derive(Clone)]
pub struct UpdateParameters<'a> {
    pub db: &'a DatabaseConnection,
    pub blockscout: &'a DatabaseConnection,
    pub blockscout_applied_migrations: BlockscoutMigrations,
    /// If `None`, it will be measured at the start of update
    /// (i.e. after taking mutexes)
    pub update_time_override: Option<chrono::DateTime<Utc>>,
    /// Force full re-update
    pub force_full: bool,
}

#[derive(Clone)]
pub struct UpdateContext<'a> {
    pub db: &'a DatabaseConnection,
    pub blockscout: &'a DatabaseConnection,
    pub blockscout_applied_migrations: BlockscoutMigrations,
    /// Update time
    pub time: chrono::DateTime<Utc>,
    pub force_full: bool,
}

impl<'a> UpdateContext<'a> {
    pub fn from_params_now_or_override(value: UpdateParameters<'a>) -> Self {
        Self {
            db: value.db,
            blockscout: value.blockscout,
            blockscout_applied_migrations: value.blockscout_applied_migrations,
            time: value.update_time_override.unwrap_or_else(Utc::now),
            force_full: value.force_full,
        }
    }
}

/// if a migratoion is active, the corresponding field is `true`.
#[derive(Clone)]
pub struct BlockscoutMigrations {
    pub denormalization: bool,
}

impl BlockscoutMigrations {
    pub async fn query_from_db(blockscout: &DatabaseConnection) -> Result<Self, DbErr> {
        let mut result = Self::empty();
        if !Self::migrations_table_exists_and_available(blockscout).await? {
            warn!("No `migrations_status` table in blockscout DB was found. It's possible in pre v6.0.0 blockscout, but otherwise is a bug. \
                Check permissions if the table actually exists. The service should work fine, but some optimizations won't be applied and \
                support for older versions is likely to be dropped in the future.");
            return Ok(Self::empty());
        }
        let migrations = migrations_status::Entity::find()
            .order_by_asc(migrations_status::Column::UpdatedAt)
            .all(blockscout)
            .await?;
        for migrations_status::Model {
            migration_name,
            status,
            ..
        } in migrations
        {
            // https://github.com/blockscout/blockscout/blob/cd1f130c93a1f4fa4f359547f08b7e609620b455/apps/explorer/lib/explorer/migrator/migration_status.ex#L12
            let value = match status.as_deref() {
                Some("completed") => true,
                Some("started") | None => false,
                Some(unknown) => {
                    warn!("unknown migration status '{}'", unknown);
                    continue;
                }
            };
            result.set(&migration_name, value);
        }
        Ok(result)
    }

    async fn migrations_table_exists_and_available(
        blockscout: &DatabaseConnection,
    ) -> Result<bool, DbErr> {
        #[derive(FromQueryResult, Debug)]
        struct AvailableTable {
            #[allow(unused)]
            table_schema: String,
            #[allow(unused)]
            table_name: String,
        }

        let migrations_table_entry = AvailableTable::find_by_statement(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            "
            SELECT table_schema, table_name
            FROM information_schema.tables
            WHERE table_schema='public'
            AND table_name='migrations_status'
            ;",
        ))
        .one(blockscout)
        .await?;

        Ok(migrations_table_entry.is_some())
    }

    fn set(&mut self, migration_name: &str, value: bool) {
        #[allow(clippy::single_match)] // expected to be extended in the future
        match migration_name {
            "denormalization" => self.denormalization = value,
            _ => (),
        }
    }

    fn empty() -> Self {
        BlockscoutMigrations {
            denormalization: false,
        }
    }

    /// All known migrations are applied
    pub fn latest() -> Self {
        BlockscoutMigrations {
            denormalization: true,
        }
    }
}

pub trait Get {
    type Value;
    fn get() -> Self::Value;
}

/// Usage:
/// ```
/// # use stats::gettable_const;
/// # use crate::stats::data_source::types::Get;
/// gettable_const!(ConstName: u64 = 123);
///
/// fn get_value_example() -> u64 {
///     ConstName::get()
/// }
/// ```
#[macro_export]
macro_rules! gettable_const {
    ($name:ident: $type:ty = $value:expr) => {
        pub struct $name;
        impl $crate::data_source::types::Get for $name {
            type Value = $type;
            fn get() -> $type {
                $value
            }
        }
    };
}
