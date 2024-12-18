use std::collections::HashMap;

use blockscout_db::entity::migrations_status;
use chrono::Utc;
use sea_orm::{DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QueryOrder, Statement};
use tracing::warn;

use crate::utils::MarkedDbConnection;

#[derive(Clone)]
pub struct UpdateParameters<'a> {
    pub db: &'a MarkedDbConnection,
    pub blockscout: &'a MarkedDbConnection,
    pub blockscout_applied_migrations: BlockscoutMigrations,
    /// If `None`, it will be measured at the start of update
    /// (i.e. after taking mutexes)
    pub update_time_override: Option<chrono::DateTime<Utc>>,
    /// Force full re-update
    pub force_full: bool,
}

#[derive(Clone)]
pub struct UpdateContext<'a> {
    pub db: &'a MarkedDbConnection,
    pub blockscout: &'a MarkedDbConnection,
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
                    warn!(
                        "unknown migration status '{}' (migration name: '{}')",
                        unknown, migration_name
                    );
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

    pub const fn empty() -> Self {
        BlockscoutMigrations {
            denormalization: false,
        }
    }

    /// All known migrations are applied
    pub const fn latest() -> Self {
        BlockscoutMigrations {
            denormalization: true,
        }
    }
}

pub enum CacheValue {
    ValueString(String),
    ValueOptionF64(Option<f64>),
}

pub trait Cacheable {
    fn from_entry(entry: CacheValue) -> Option<Self>
    where
        Self: Sized;
    fn from_entry_ref(entry: &CacheValue) -> Option<&Self>
    where
        Self: Sized;
    fn into_entry(self) -> CacheValue;
}

macro_rules! impl_cacheable {
    ($type: ty, $cache_value_variant:ident) => {
        impl Cacheable for $type {
            fn from_entry(entry: CacheValue) -> Option<Self>
            where
                Self: Sized,
            {
                match entry {
                    CacheValue::$cache_value_variant(s) => Some(s),
                    _ => None,
                }
            }

            fn from_entry_ref(entry: &CacheValue) -> Option<&Self>
            where
                Self: Sized,
            {
                match entry {
                    CacheValue::$cache_value_variant(s) => Some(s),
                    _ => None,
                }
            }

            fn into_entry(self) -> CacheValue {
                CacheValue::$cache_value_variant(self)
            }
        }
    };
}

impl_cacheable!(String, ValueString);
impl_cacheable!(Option<f64>, ValueOptionF64);

pub struct UpdateCache {
    inner: HashMap<String, CacheValue>,
}

impl UpdateCache {
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }
}

pub trait StorageFor<V> {
    fn insert(&mut self, query: &Statement, value: V) -> Option<V>;

    fn get(&mut self, query: &Statement) -> Option<&V>;
}

impl UpdateCache {
    /// If the cache did not have value for this query present, None is returned.
    ///
    /// If the cache did have this query present, the value is updated, and the old value is returned.
    fn insert<V: Cacheable>(&mut self, query: &Statement, value: V) -> Option<V> {
        self.inner
            .insert(query.to_string(), value.into_entry())
            .and_then(|e| V::from_entry(e))
    }

    /// Returns reference to a value for this query, if present
    fn get<V: Cacheable>(&mut self, query: &Statement) -> Option<&V> {
        self.inner
            .get(&query.to_string())
            .and_then(|e| V::from_entry_ref(e))
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

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use sea_orm::DbBackend;

    use super::*;

    #[test]
    fn cache_works() {
        let mut cache = UpdateCache::new();
        let stmt_a = Statement::from_string(DbBackend::Sqlite, "abcde");
        let stmt_b = Statement::from_string(DbBackend::Sqlite, "edcba");

        let val_1 = Some(1.2);
        let val_2 = "kekekek".to_string();

        cache.insert::<Option<f64>>(&stmt_a, val_1);
        assert_eq!(cache.get::<Option<f64>>(&stmt_a), Some(&val_1));
        assert_eq!(cache.get::<String>(&stmt_a), None);

        cache.insert::<Option<f64>>(&stmt_a, None);
        assert_eq!(cache.get::<Option<f64>>(&stmt_a), Some(&None));
        assert_eq!(cache.get::<String>(&stmt_a), None);

        cache.insert::<String>(&stmt_a, val_2.clone());
        assert_eq!(cache.get::<Option<f64>>(&stmt_a), None);
        assert_eq!(cache.get::<String>(&stmt_a), Some(&val_2));

        cache.insert::<Option<f64>>(&stmt_b, val_1);
        assert_eq!(cache.get::<Option<f64>>(&stmt_b), Some(&val_1));
        assert_eq!(cache.get::<String>(&stmt_b), None);
        assert_eq!(cache.get::<Option<f64>>(&stmt_a), None);
        assert_eq!(cache.get::<String>(&stmt_a), Some(&val_2));
    }
}
