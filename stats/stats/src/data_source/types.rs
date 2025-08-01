use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use blockscout_db::entity::migrations_status;
use chrono::Utc;
use sea_orm::{
    DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QueryOrder, Statement, TryGetable,
};
use tokio::sync::Mutex;
use tracing::warn;

use crate::{ChartKey, counters::TxnsStatsValue, types::new_txns::NewTxnsCombinedPoint};

#[derive(Clone)]
pub struct UpdateParameters<'a> {
    pub stats_db: &'a DatabaseConnection,
    /// `true` - the connection is to multichain indexer DB;
    /// `false` - the connection is to usual (per-instance) blockscout DB.
    pub is_multichain_mode: bool,
    /// Indexer database (blockscout or multichain)
    pub indexer_db: &'a DatabaseConnection,
    pub indexer_applied_migrations: IndexerMigrations,
    /// Charts engaged in the current (group) update.
    /// Includes recursively affected charts.
    pub enabled_update_charts_recursive: HashSet<ChartKey>,
    /// If `None`, it will be measured at the start of update
    /// (i.e. after taking mutexes)
    pub update_time_override: Option<chrono::DateTime<Utc>>,
    /// Force full re-update
    pub force_full: bool,
}

impl<'a> UpdateParameters<'a> {
    /// Parameter builder for just querying data (if no updates are expected)
    pub fn query_parameters(
        db: &'a DatabaseConnection,
        is_multichain_mode: bool,
        indexer: &'a DatabaseConnection,
        indexer_applied_migrations: IndexerMigrations,
        query_time_override: Option<chrono::DateTime<Utc>>,
    ) -> Self {
        Self {
            stats_db: db,
            is_multichain_mode,
            indexer_db: indexer,
            indexer_applied_migrations,
            update_time_override: query_time_override,
            // not an update, therefore empty.
            // also it's used for reusing queries, but
            // non-update queries must not be expensive;
            // therefore it's not needed in this case
            enabled_update_charts_recursive: HashSet::new(),
            // doesn't make sense during query
            force_full: false,
        }
    }
}

#[derive(Clone)]
pub struct UpdateContext<'a> {
    pub stats_db: &'a DatabaseConnection,
    pub is_multichain_mode: bool,
    /// Indexer database (blockscout or multichain depending on `is_multichain_mode`)
    pub indexer_db: &'a DatabaseConnection,
    pub indexer_applied_migrations: IndexerMigrations,
    pub cache: UpdateCache,
    /// Charts engaged in the current (group) update.
    /// Includes recursively affected charts.
    pub enabled_update_charts_recursive: HashSet<ChartKey>,
    /// Update time
    pub time: chrono::DateTime<Utc>,
    pub force_full: bool,
}

impl<'a> UpdateContext<'a> {
    pub fn from_params_now_or_override(value: UpdateParameters<'a>) -> Self {
        Self {
            stats_db: value.stats_db,
            is_multichain_mode: value.is_multichain_mode,
            indexer_db: value.indexer_db,
            indexer_applied_migrations: value.indexer_applied_migrations,
            cache: UpdateCache::new(),
            enabled_update_charts_recursive: value.enabled_update_charts_recursive,
            time: value.update_time_override.unwrap_or_else(Utc::now),
            force_full: value.force_full,
        }
    }
}

/// if a migratoion is active, the corresponding field is `true`.
#[derive(Clone)]
pub struct IndexerMigrations {
    pub denormalization: bool,
}

impl IndexerMigrations {
    pub async fn query_from_db(
        is_multichain: bool,
        indexer: &DatabaseConnection,
    ) -> Result<Self, DbErr> {
        if is_multichain {
            Ok(Self::empty())
        } else {
            Self::query_from_blockscout_db(indexer).await
        }
    }

    pub async fn query_from_blockscout_db(indexer: &DatabaseConnection) -> Result<Self, DbErr> {
        let mut result = Self::empty();
        if !Self::blockscout_migrations_table_exists_and_available(indexer).await? {
            warn!(
                "No `migrations_status` table in blockscout DB was found. It's possible in pre v6.0.0 blockscout, but otherwise is a bug. \
                Check permissions if the table actually exists. The service should work fine, but some optimizations won't be applied and \
                support for older versions is likely to be dropped in the future."
            );
            return Ok(Self::empty());
        }
        let migrations = migrations_status::Entity::find()
            .order_by_asc(migrations_status::Column::UpdatedAt)
            .all(indexer)
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

    async fn blockscout_migrations_table_exists_and_available(
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
        IndexerMigrations {
            denormalization: false,
        }
    }

    /// All known migrations are applied
    pub const fn latest() -> Self {
        IndexerMigrations {
            denormalization: true,
        }
    }
}

#[derive(Clone, Debug)]
pub enum CacheValue {
    ValueString(String),
    ValueOptionF64(Option<f64>),
    ValueTxnsStats(TxnsStatsValue),
    ValueNewTxnsCombined(NewTxnsCombinedPoint),
    VecTxnWindow(Vec<NewTxnsCombinedPoint>),
}

pub trait Cacheable {
    fn from_entry(entry: CacheValue) -> Option<Self>
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

            fn into_entry(self) -> CacheValue {
                CacheValue::$cache_value_variant(self)
            }
        }
    };
}

impl_cacheable!(TxnsStatsValue, ValueTxnsStats);
impl_cacheable!(NewTxnsCombinedPoint, ValueNewTxnsCombined);
impl_cacheable!(Vec<NewTxnsCombinedPoint>, VecTxnWindow);
// for testing
impl_cacheable!(String, ValueString);
impl_cacheable!(Option<f64>, ValueOptionF64);

// To allow using the scalar(?) types in context requiring
// `FromQueryResult`
#[derive(Debug, Clone, FromQueryResult, PartialEq, Eq, PartialOrd, Ord)]
pub struct WrappedValue<V: TryGetable> {
    pub value: V,
}

impl<V: TryGetable> From<V> for WrappedValue<V> {
    fn from(value: V) -> Self {
        WrappedValue { value }
    }
}

impl<V: TryGetable> WrappedValue<V> {
    pub fn into_inner(self) -> V {
        self.value
    }
}

impl<V: TryGetable + Copy> Copy for WrappedValue<V> {}

macro_rules! impl_cacheable_wrapped {
    ($type: ty, $cache_value_variant:ident) => {
        impl Cacheable for $type {
            fn from_entry(entry: CacheValue) -> Option<Self>
            where
                Self: Sized,
            {
                match entry {
                    CacheValue::$cache_value_variant(s) => Some(WrappedValue { value: s }),
                    _ => None,
                }
            }

            fn into_entry(self) -> CacheValue {
                CacheValue::$cache_value_variant(self.value)
            }
        }
    };
}

impl_cacheable_wrapped!(WrappedValue<String>, ValueString);
impl_cacheable_wrapped!(WrappedValue<Option<f64>>, ValueOptionF64);

/// There is no cache invalidation logic, because the cache is
/// expected to be constructed from scratch on each group update
/// and dropped after the update.
///
/// Also see a [`crate::construct_update_group!`] implementation
#[derive(Clone, Debug, Default)]
pub struct UpdateCache {
    inner: Arc<Mutex<HashMap<String, CacheValue>>>,
}

impl UpdateCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl UpdateCache {
    /// If the cache did not have value for this query present, None is returned.
    ///
    /// If the cache did have this query present, the value is updated, and the old value is returned.
    pub async fn insert<V: Cacheable>(&self, query: &Statement, value: V) -> Option<V> {
        self.inner
            .lock()
            .await
            .insert(query.to_string(), value.into_entry())
            .and_then(|e| V::from_entry(e))
    }

    /// Returns a value for this query, if present
    pub async fn get<V: Cacheable>(&self, query: &Statement) -> Option<V> {
        self.inner
            .lock()
            .await
            .get(&query.to_string())
            .and_then(|e| V::from_entry(e.clone()))
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

    #[tokio::test]
    async fn cache_works() {
        let cache = UpdateCache::new();
        let stmt_a = Statement::from_string(DbBackend::Sqlite, "abcde");
        let stmt_b = Statement::from_string(DbBackend::Sqlite, "edcba");

        let val_1 = Some(1.2).into();
        let val_2 = "kekekek".to_string();

        cache
            .insert::<WrappedValue<Option<f64>>>(&stmt_a, val_1)
            .await;
        assert_eq!(
            cache.get::<WrappedValue<Option<f64>>>(&stmt_a).await,
            Some(val_1)
        );
        assert_eq!(cache.get::<String>(&stmt_a).await, None);

        cache.insert::<Option<f64>>(&stmt_a, None).await;
        assert_eq!(cache.get::<Option<f64>>(&stmt_a).await, Some(None));
        assert_eq!(cache.get::<String>(&stmt_a).await, None);

        cache.insert::<String>(&stmt_a, val_2.clone()).await;
        assert_eq!(cache.get::<Option<f64>>(&stmt_a).await, None);
        assert_eq!(cache.get::<String>(&stmt_a).await, Some(val_2.clone()));

        cache
            .insert::<WrappedValue<Option<f64>>>(&stmt_b, val_1)
            .await;
        assert_eq!(
            cache.get::<WrappedValue<Option<f64>>>(&stmt_b).await,
            Some(val_1)
        );
        assert_eq!(cache.get::<String>(&stmt_b).await, None);
        assert_eq!(cache.get::<Option<f64>>(&stmt_a).await, None);
        assert_eq!(cache.get::<String>(&stmt_a).await, Some(val_2));
    }
}
