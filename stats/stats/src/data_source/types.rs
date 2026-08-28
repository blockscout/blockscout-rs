// SPDX-License-Identifier: LicenseRef-Blockscout

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use blockscout_db::entity::migrations_status;
use chrono::{NaiveDate, NaiveDateTime, Utc};
use sea_orm::{
    DatabaseConnection, DbErr, EntityTrait, FromQueryResult, QueryOrder, Statement, TryGetable,
};
use tokio::sync::Mutex;
use tracing::warn;

use crate::{
    ChartKey,
    charts::db_interaction::filters::interchain::InterchainFilter,
    counters::{FilecoinChainFees24hValue, TxnsStatsValue},
    mode::Mode,
    types::new_txns::NewTxnsCombinedPoint,
};

/// Process-local memory of interchain "trigger 2" (stored-floor regression)
/// rebuilds that produced no movement of the chart's stored floor.
///
/// **Why this exists.** `interchain_history_floor_regressed`
/// (`data_source::kinds::local_db`) compares a chart's own stored floor
/// against the shared, filter-scoped indexer floor
/// (`get_min_date`/`get_min_date_interchain`) — which is `min` of a
/// message-filtered and a transfer-filtered floor, not anything scoped to the
/// chart's own predicate (see `.memory-bank/gotchas.md` → "A Transfer's Token
/// Chains Are Not Its Message's Route"). A chart whose own predicate matches no
/// rows between the two floors therefore regresses forever: every rebuild it
/// triggers inserts nothing in the gap, so its stored floor never moves, and
/// the identical comparison fires again next cycle. This memory remembers the
/// exact `(stored_floor, indexer_floor)` pair a rebuild already proved
/// unproductive for a chart, so `local_db::update_itself_inner` can suppress a
/// byte-for-byte repeat of it — see the call site there for exactly which of
/// the two rebuild paths (a chart's own comparison vs. the propagated daily
/// sibling marker) this applies to.
///
/// **Process-local, deliberately.** This is an in-memory `HashMap`, not a
/// stats-DB column: it is lost on every process restart, so the worst case
/// this reintroduces is one repeated unproductive rebuild per affected chart
/// per process start — bounded and cheap. The alternative (a persisted column)
/// would need a migration for a problem that only costs a handful of wasted
/// full recomputes over a process's lifetime; not worth it.
///
/// `Clone` is cheap: the map lives behind an `Arc<Mutex<_>>`, so every clone
/// shares the same underlying memory. One instance is owned by `UpdateService`
/// and threaded into every group's `UpdateParameters` for the life of the
/// process.
#[derive(Clone, Default)]
pub struct InterchainBackfillMemory {
    /// `chart -> (stored_floor, indexer_floor)` of the last rebuild that this
    /// chart triggered on its own floor-regression comparison and that did NOT
    /// move the chart's stored floor.
    unproductive: Arc<Mutex<HashMap<ChartKey, (NaiveDate, NaiveDate)>>>,
}

impl InterchainBackfillMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` if `pair` is already known, for `chart`, to produce no
    /// stored-floor movement when rebuilt — i.e. triggering on it again would
    /// only repeat a rebuild already proven to insert nothing new.
    pub async fn is_known_unproductive(
        &self,
        chart: &ChartKey,
        pair: (NaiveDate, NaiveDate),
    ) -> bool {
        self.unproductive.lock().await.get(chart) == Some(&pair)
    }

    /// Record `pair` as unproductive for `chart`: the rebuild it just
    /// triggered did not move the chart's stored floor. Overwrites any
    /// previously recorded pair for this chart, which is exactly what is
    /// wanted when the pair itself has changed (e.g. the indexer floor moved
    /// further, but the chart's own floor still didn't) — the new pair is what
    /// must be proven unproductive again before being suppressed.
    pub async fn record_unproductive(&self, chart: ChartKey, pair: (NaiveDate, NaiveDate)) {
        self.unproductive.lock().await.insert(chart, pair);
    }

    /// Clear any remembered unproductive pair for `chart` — its stored floor
    /// moved, so a previously recorded pair (if any) no longer describes a
    /// standing situation and must not suppress a future, different
    /// regression.
    pub async fn clear(&self, chart: &ChartKey) {
        self.unproductive.lock().await.remove(chart);
    }
}

#[derive(Clone)]
pub struct UpdateParameters<'a> {
    pub stats_db: &'a DatabaseConnection,
    /// Service mode (from settings); determines indexer type and query branching.
    pub mode: Mode,
    /// Chain IDs to filter by in MultichainAggregator mode
    pub multichain_filter: Option<Vec<u64>>,
    /// Read filter applied to the interchain indexer DB, fully resolved for this
    /// update cycle (operator configuration + the observability horizon).
    pub interchain_filter: InterchainFilter,
    /// Indexer database (blockscout, multichain, or interchain)
    pub indexer_db: &'a DatabaseConnection,
    pub indexer_applied_migrations: IndexerMigrations,
    /// Second indexer database (CCTX indexer currently)
    pub second_indexer_db: Option<&'a DatabaseConnection>,
    /// Charts engaged in the current (group) update.
    /// Includes recursively affected charts.
    pub enabled_update_charts_recursive: HashSet<ChartKey>,
    /// If `None`, it will be measured at the start of update
    /// (i.e. after taking mutexes)
    pub update_time_override: Option<chrono::DateTime<Utc>>,
    /// Force full re-update
    pub force_full: bool,
    /// Process-local memory suppressing a repeat interchain trigger-2 rebuild
    /// already proven unproductive — see [`InterchainBackfillMemory`].
    ///
    /// `None` means "no memory available": `local_db::update_itself_inner`
    /// then behaves exactly as it did before this memory existed (every floor
    /// regression triggers, unconditionally). This is what `query_parameters`
    /// and every non-interchain-specific test construction site pass, since
    /// none of them exercise the suppression this memory enables.
    pub interchain_backfill_memory: Option<InterchainBackfillMemory>,
}

impl<'a> UpdateParameters<'a> {
    /// Parameter builder for just querying data (if no updates are expected)
    /// Query parameters are just a subset of the update parameters,
    /// which is why there are a few fields that are not applicable to query parameters.
    /// Build parameters for reading stored chart data. Filter fields like
    /// `multichain_filter` and `interchain_filter` are not used when reading.
    pub fn query_parameters(
        db: &'a DatabaseConnection,
        indexer: &'a DatabaseConnection,
        indexer_applied_migrations: IndexerMigrations,
        second_indexer: Option<&'a DatabaseConnection>,
        query_time_override: Option<chrono::DateTime<Utc>>,
        mode: Mode,
    ) -> Self {
        Self {
            stats_db: db,
            mode,
            multichain_filter: None, // only used when updating the DB
            // only used when updating the DB
            interchain_filter: InterchainFilter::default(),
            indexer_db: indexer,
            indexer_applied_migrations,
            second_indexer_db: second_indexer,
            update_time_override: query_time_override,
            // not an update, therefore empty.
            // also it's used for reusing queries, but
            // non-update queries must not be expensive;
            // therefore it's not needed in this case
            enabled_update_charts_recursive: HashSet::new(),
            // doesn't make sense during query
            force_full: false,
            // only used when updating the DB
            interchain_backfill_memory: None,
        }
    }
}

impl<'a> UpdateParameters<'a> {
    /// Attach [`InterchainBackfillMemory`] to these parameters. Without this,
    /// `interchain_backfill_memory` stays `None` and
    /// `local_db::update_itself_inner` never suppresses a repeat trigger-2
    /// rebuild — see the field's doc comment.
    pub fn with_interchain_backfill_memory(mut self, memory: InterchainBackfillMemory) -> Self {
        self.interchain_backfill_memory = Some(memory);
        self
    }
}

#[cfg(test)]
impl<'a> UpdateParameters<'a> {
    /// Default parameters for blockscout stats & latest migrations
    pub fn default_test_parameters(
        db: &'a DatabaseConnection,
        indexer: &'a DatabaseConnection,
        enabled_charts_recursive: HashSet<ChartKey>,
        time_override: Option<chrono::DateTime<Utc>>,
    ) -> Self {
        Self {
            stats_db: db,
            mode: Mode::Blockscout,
            multichain_filter: None,
            interchain_filter: InterchainFilter::default(),
            indexer_db: indexer,
            indexer_applied_migrations: IndexerMigrations::latest(),
            second_indexer_db: None,
            update_time_override: time_override,
            enabled_update_charts_recursive: enabled_charts_recursive,
            force_full: false,
            interchain_backfill_memory: None,
        }
    }

    /// Default parameters for querying blockscout stats (w/ latest migrations)
    pub fn default_test_query_parameters(
        db: &'a DatabaseConnection,
        indexer: &'a DatabaseConnection,
        time_override: Option<chrono::DateTime<Utc>>,
    ) -> Self {
        UpdateParameters::query_parameters(
            db,
            indexer,
            IndexerMigrations::latest(),
            None,
            time_override,
            Mode::Blockscout,
        )
    }

    pub fn with_force_full(mut self) -> Self {
        self.force_full = true;
        self
    }
}

#[derive(Clone)]
pub struct UpdateContext<'a> {
    pub stats_db: &'a DatabaseConnection,
    pub mode: Mode,
    pub multichain_filter: Option<Vec<u64>>,
    /// Read filter applied to the interchain indexer DB, fully resolved for this
    /// update cycle (operator configuration + the observability horizon).
    pub interchain_filter: InterchainFilter,
    /// Indexer database (blockscout, multichain, or interchain depending on mode)
    pub indexer_db: &'a DatabaseConnection,
    pub indexer_applied_migrations: IndexerMigrations,
    pub second_indexer_db: Option<&'a DatabaseConnection>,
    pub cache: UpdateCache,
    /// Charts engaged in the current (group) update.
    /// Includes recursively affected charts.
    pub enabled_update_charts_recursive: HashSet<ChartKey>,
    /// Update time
    pub time: chrono::DateTime<Utc>,
    pub force_full: bool,
    /// See [`InterchainBackfillMemory`] and the field of the same name on
    /// [`UpdateParameters`], which this is copied from verbatim.
    pub interchain_backfill_memory: Option<InterchainBackfillMemory>,
}

impl<'a> UpdateContext<'a> {
    pub fn from_params_now_or_override(value: UpdateParameters<'a>) -> Self {
        Self {
            stats_db: value.stats_db,
            mode: value.mode,
            multichain_filter: value.multichain_filter,
            interchain_filter: value.interchain_filter,
            indexer_db: value.indexer_db,
            indexer_applied_migrations: value.indexer_applied_migrations,
            second_indexer_db: value.second_indexer_db,
            cache: UpdateCache::new(),
            enabled_update_charts_recursive: value.enabled_update_charts_recursive,
            time: value.update_time_override.unwrap_or_else(Utc::now),
            force_full: value.force_full,
            interchain_backfill_memory: value.interchain_backfill_memory,
        }
    }
}

/// if a migratoion is active, the corresponding field is `true`.
#[derive(Clone)]
pub struct IndexerMigrations {
    pub denormalization: bool,
}

impl IndexerMigrations {
    pub async fn query_from_db(mode: Mode, indexer: &DatabaseConnection) -> Result<Self, DbErr> {
        match mode {
            Mode::Blockscout | Mode::Zetachain => Self::query_from_blockscout_db(indexer).await,
            _ => Ok(Self::empty()),
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
    ValueOptionNaiveDateTime(Option<NaiveDateTime>),
    ValueTxnsStats(TxnsStatsValue),
    ValueFilecoinChainFees24h(FilecoinChainFees24hValue),
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
impl_cacheable!(FilecoinChainFees24hValue, ValueFilecoinChainFees24h);
impl_cacheable!(NewTxnsCombinedPoint, ValueNewTxnsCombined);
impl_cacheable!(Vec<NewTxnsCombinedPoint>, VecTxnWindow);
// for testing
impl_cacheable!(String, ValueString);
impl_cacheable!(Option<f64>, ValueOptionF64);
impl_cacheable!(Option<NaiveDateTime>, ValueOptionNaiveDateTime);

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

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[tokio::test]
    async fn interchain_backfill_memory_tracks_unproductive_pairs_per_chart() {
        let memory = InterchainBackfillMemory::new();
        let chart_a = ChartKey::with_day("chart_a".to_owned());
        let chart_b = ChartKey::with_day("chart_b".to_owned());
        let pair = (d("2023-02-01"), d("2023-01-01"));
        let other_pair = (d("2023-02-01"), d("2022-12-01"));

        // nothing recorded yet for either chart
        assert!(!memory.is_known_unproductive(&chart_a, pair).await);
        assert!(!memory.is_known_unproductive(&chart_b, pair).await);

        memory.record_unproductive(chart_a.clone(), pair).await;
        assert!(memory.is_known_unproductive(&chart_a, pair).await);
        // a different pair for the same chart is not the recorded one
        assert!(!memory.is_known_unproductive(&chart_a, other_pair).await);
        // a different chart is unaffected by chart_a's record
        assert!(!memory.is_known_unproductive(&chart_b, pair).await);

        // recording a new pair for the same chart overwrites the old one
        memory
            .record_unproductive(chart_a.clone(), other_pair)
            .await;
        assert!(memory.is_known_unproductive(&chart_a, other_pair).await);
        assert!(!memory.is_known_unproductive(&chart_a, pair).await);

        // clearing removes the record entirely, not just makes it stale
        memory.clear(&chart_a).await;
        assert!(!memory.is_known_unproductive(&chart_a, other_pair).await);

        // clearing a chart with no record is a harmless no-op
        memory.clear(&chart_b).await;
    }
}
