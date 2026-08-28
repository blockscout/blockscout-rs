// SPDX-License-Identifier: LicenseRef-Blockscout

//! Source that is persisted in local database.
//!
//! Such sources are the only ones (so far) that
//! change their state during update.
//! For example, remote sources are updated independently from
//! this service, and sources from data manipulation only transform
//! some other source's data on query.
//!
//! Charts are intended to be such persisted sources,
//! because their data is directly retreived from the database (on requests).

use std::{fmt::Debug, marker::PhantomData, time::Duration};

use blockscout_metrics_tools::AggregateTimer;
use chrono::{DateTime, SubsecRound, Utc};
use parameter_traits::{CreateBehaviour, QueryBehaviour, UpdateBehaviour};
use parameters::{
    DefaultCreate, DefaultQueryLast, DefaultQueryVec, QueryLastWithEstimationFallback,
    update::{
        batching::{
            BatchUpdate,
            parameters::{AddLastValueStep, Batch30Days, PassVecStep},
        },
        point::PassPoint,
    },
};
use sea_orm::{DatabaseConnection, DbBackend, DbErr, Statement};

use crate::{
    ChartError, ChartKey, IndexingStatus, Mode,
    charts::{
        ChartProperties, Named, ResolutionKind, chart_properties_portrait,
        db_interaction::{
            read::{
                get_chart_metadata, get_min_block_blockscout, get_min_date,
                interchain::get_min_block_interchain, last_accurate_point,
                multichain::get_min_block_multichain, recorded_min_chart_date,
                recorded_min_indexer_block,
            },
            write::{clear_chart_data_and_updated_at, set_last_updated_at},
        },
    },
    data_source::{
        DataSource, UpdateContext, kinds::local_db::cached::RemoteCachedLocalDbChartSource,
    },
    metrics,
    range::UniversalRange,
    types::Timespan,
    utils::day_start,
};
use entity::sea_orm_active_enums::ChartType;

use super::auxiliary::PartialCumulative;

pub mod cached;
pub mod parameter_traits;
pub mod parameters;

/// The source is configurable in many aspects. In particular,
/// - dependencies
/// - implementation of CRUD (without D) (=behaviour)
/// - chart settings/properties
///
///
/// There are types that implement each of the behaviour type in
/// [`parameters`]; also there are type aliases in [`self`]
/// with common parameter combinations.
///
/// See [module-level documentation](self) for more details.
pub struct LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>(
    pub PhantomData<(MainDep, ResolutionDep, Create, Update, Query, ChartProps)>,
)
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    Create: CreateBehaviour,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution>,
    Query: QueryBehaviour,
    ChartProps: ChartProperties;

// not in `data_manipulation` because it requires retrieving latest (self) value before
// next batch
/// Chart with cumulative data calculated from delta dependency
/// (dependency with changes from previous point == increments+decrements or deltas)
///
/// So, if the values of `NewItemsChart` are [1, 2, 3, 4], then
/// cumulative chart will produce [1, 3, 6, 10].
///
/// Missing points in dependency's output are expected to mean zero value
/// (==`MissingDatePolicy::FillZero`).
/// [see "Dependency requirements" here](crate::data_source::kinds)
///
/// The opposite logic to [`Delta`](`crate::data_source::kinds::data_manipulation::delta::Delta`)
pub type DailyCumulativeLocalDbChartSource<DeltaDep, C> = LocalDbChartSource<
    PartialCumulative<DeltaDep>,
    (),
    DefaultCreate<C>,
    BatchUpdate<
        PartialCumulative<DeltaDep>,
        (),
        AddLastValueStep<C>,
        Batch30Days,
        DefaultQueryVec<C>,
        C,
    >,
    DefaultQueryVec<C>,
    C,
>;

/// Chart that stores vector data received from provided dependency (without
/// any manipulations)
pub type DirectVecLocalDbChartSource<Dependency, BatchSizeUpperBound, C> = LocalDbChartSource<
    Dependency,
    (),
    DefaultCreate<C>,
    BatchUpdate<Dependency, (), PassVecStep, BatchSizeUpperBound, DefaultQueryVec<C>, C>,
    DefaultQueryVec<C>,
    C,
>;

/// Chart that stores single data point received from provided dependency (without
/// any manipulations)
pub type DirectPointLocalDbChartSource<Dependency, C> = LocalDbChartSource<
    Dependency,
    (),
    DefaultCreate<C>,
    PassPoint<Dependency>,
    DefaultQueryLast<C>,
    C,
>;

pub type DirectPointLocalDbChartSourceWithEstimate<Dependency, Estimate, C> = LocalDbChartSource<
    Dependency,
    (),
    DefaultCreate<C>,
    PassPoint<Dependency>,
    QueryLastWithEstimationFallback<Estimate, C>,
    C,
>;

pub type DirectPointCachedLocalDbChartSource<Dependency, CacheTimeout, C> =
    RemoteCachedLocalDbChartSource<
        Dependency,
        (),
        DefaultCreate<C>,
        PassPoint<Dependency>,
        DefaultQueryLast<C>,
        CacheTimeout,
        C,
    >;

impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
    LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource + Sync,
    ResolutionDep: DataSource + Sync,
    Create: CreateBehaviour + Sync,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution> + Sync,
    Query: QueryBehaviour + Sync,
    ChartProps: ChartProperties,
    ChartProps::Resolution: Ord + Clone + Debug,
{
    /// Performs common checks and prepares values useful for further
    /// update. Then proceeds to update according to parameters.
    async fn update_itself_inner(
        cx: &UpdateContext<'_>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), ChartError> {
        let metadata = get_chart_metadata(cx.stats_db, &ChartProps::key()).await?;
        if let Some(last_updated_at) = metadata.last_updated_at {
            if postgres_timestamps_eq(cx.time, last_updated_at) {
                // no need to perform update.
                // mostly catches second call to update e.g. when both
                // dependency and this source are in one group and enabled.
                tracing::debug!(
                    last_updated_at =? last_updated_at,
                    update_timestamp =? cx.time,
                    "Not updating the chart because it was already handled within ongoing update"
                );
                return Ok(());
            } else {
                tracing::debug!(
                    last_updated_at =? last_updated_at,
                    update_timestamp =? cx.time,
                    "Performing an update"
                );
            }
        }
        let chart_id = metadata.id;
        let min_indexer_block = match cx.mode {
            Mode::Interchain => get_min_block_interchain(&cx.interchain_filter)
                .await
                .map_err(ChartError::IndexerDB)?,
            Mode::MultichainAggregator => get_min_block_multichain(cx.indexer_db)
                .await
                .map_err(ChartError::IndexerDB)?,
            Mode::Blockscout | Mode::Zetachain => get_min_block_blockscout(cx.indexer_db)
                .await
                .map_err(ChartError::IndexerDB)?,
        };
        // Interchain only, where `min_indexer_block` is the filter fingerprint
        // rather than a block number. `last_accurate_point` below already forces a
        // full recompute when the recorded fingerprint differs, but
        // `insert_data_many` is an upsert on (chart_id, date) with no delete
        // (`write.rs`) — so a *narrowed* filter would leave stale non-zero rows on
        // days that now yield no row at all. Those days read as `0` under
        // `FillZero`, and for the cumulative `messagesGrowth*` series a stale
        // prefix propagates through everything after it. Delete first.
        //
        // Deliberately NOT gated on `!cx.force_full`: if the fingerprint changed
        // the delete is wanted either way, and `force_full` skips
        // `last_accurate_point`'s read but not the staleness problem. Deliberately
        // NOT extended to other modes: there `min_blockscout_block` means an actual
        // block number and the existing reindex semantics depend on it.
        // Read once per chart per cycle. Both the interchain gate just below and
        // `last_accurate_point` further down need this value, and each used to
        // query it separately — the same row, twice. Skipped only in the one
        // combination where nobody looks at it: `force_full` makes
        // `last_accurate_point` ignore it, and outside interchain mode there is no
        // gate.
        let recorded = if cx.force_full && cx.mode != Mode::Interchain {
            None
        } else {
            recorded_min_indexer_block(cx.stats_db, chart_id).await?
        };
        if cx.mode == Mode::Interchain
            // no stored rows ⇒ nothing to clear. A NULL recorded value counts as a
            // mismatch: interchain never writes one, so this should be
            // unreachable, and treating it as a mismatch is the safe direction.
            && let Some(recorded) = recorded
            && recorded != Some(min_indexer_block)
        {
            tracing::warn!(
                chart =% ChartProps::key(),
                recorded =? recorded,
                observed = min_indexer_block,
                "interchain filter fingerprint changed; clearing stored chart data \
                 before recomputing"
            );
            // Clears `last_updated_at` together with the rows, in one transaction.
            // `update_metadata` below runs only if `update_values` succeeds, so a
            // rebuild interrupted in between (indexer hiccup, pod eviction — and the
            // window is at its widest on the cycle that first applies a new filter,
            // when all interchain charts rebuild at once) would otherwise leave the
            // chart empty while still carrying its pre-clear freshness: an empty
            // series that nothing reports as stale. Interchain-only, like the
            // fingerprint itself; every other mode keeps reaching
            // `clear_all_chart_data` through the in-place paths only.
            clear_chart_data_and_updated_at(cx.stats_db, chart_id)
                .await
                .map_err(ChartError::StatsDB)?;
        }
        // Trigger 2 ("stored-floor check"): a line chart whose earliest stored
        // date sits above the indexer's current filtered floor picks up history
        // the indexer has backfilled, without a clear. Each guard term is
        // load-bearing:
        // - `!cx.force_full` — a rebuild is already happening, so skip both reads
        //   (this also means a trigger-1 cycle never pays for trigger 2);
        // - `cx.mode == Mode::Interchain` — the other modes detect backfill
        //   through the real `min_blockscout_block` and must not change;
        // - `ChartType::Line` — counters store one point stamped at the
        //   *current* timespan, so their floor is always today and an ungated
        //   check would fire on every counter, every cycle, forever.
        let interchain_line_gate = !cx.force_full
            && cx.mode == Mode::Interchain
            && ChartProps::chart_type() == ChartType::Line;
        let raw_floor_regressed = if interchain_line_gate {
            interchain_history_floor_regressed::<ChartProps>(cx, chart_id).await?
        } else {
            None
        };
        // Suppress a *repeat* of an already-proven-unproductive trigger-2
        // rebuild. A chart's own predicate can match no rows in the gap
        // between its stored floor and the shared, filter-scoped indexer floor
        // (see `InterchainBackfillMemory`'s doc comment for why this arises —
        // in short, the shared floor is `min` of a message- and a
        // transfer-filtered floor, neither of which is the chart's own
        // predicate). A rebuild triggered by such a gap never moves the
        // chart's stored floor, so without this check the identical
        // comparison fires again next cycle, forever — a full rebuild every
        // cycle for no benefit. Comparing the exact `(stored_floor,
        // indexer_floor)` pair — not just "did it regress" — is what lets a
        // *genuine* further regression (the indexer floor moving even lower)
        // through unsuppressed: that is a different pair, unproven, and must
        // trigger.
        //
        // This suppression applies ONLY to this chart's own comparison above
        // (`raw_floor_regressed`) — never to `daily_sibling_rebuilt` below.
        // That path exists precisely because a lower resolution's own
        // comparison is blind to an intra-bucket floor movement (see the
        // "Propagation fix" comment just below); it is driven by whether the
        // *daily* chart's own comparison fired this cycle, which this memory
        // has no say over. A lower resolution's own memory (if it even has an
        // entry — resolution charts hit trigger 2 far less often, per that
        // same "Propagation fix" blind spot) must never suppress a rebuild the
        // daily sibling just proved warranted.
        // Matched by value (not by reference) so nothing borrowed from
        // `raw_floor_regressed`/`stored_floor`/`indexer_floor` is held across
        // the `.await` below — `ChartProps::Resolution` isn't required to be
        // `Sync`, only `Send`, and a held reference would need the former for
        // the surrounding future to stay `Send`.
        let own_floor_regressed =
            match (raw_floor_regressed, cx.interchain_backfill_memory.as_ref()) {
                (Some((stored_floor, indexer_floor)), Some(memory)) => {
                    let pair = (stored_floor.into_date(), indexer_floor.into_date());
                    if memory.is_known_unproductive(&ChartProps::key(), pair).await {
                        tracing::debug!(
                            chart =% ChartProps::key(),
                            stored_floor =? pair.0,
                            indexer_floor =? pair.1,
                            "suppressing interchain floor-regression rebuild: this exact pair \
                             already produced no stored-floor movement earlier in this process"
                        );
                        None
                    } else {
                        Some((
                            ChartProps::Resolution::from_date(pair.0),
                            ChartProps::Resolution::from_date(pair.1),
                        ))
                    }
                }
                // no memory configured (e.g. `query_parameters`/most test
                // construction sites) ⇒ behave exactly as before this memory
                // existed, or the raw comparison found no regression to begin with
                (raw, _) => raw,
            };
        // Propagation fix: both sides of `interchain_history_floor_regressed`'s
        // comparison go through `ChartProps::Resolution::from_date`, and for a
        // resolution chart a stored `date` already **is** the bucket's first day
        // (`Week::into_date() == saturating_first_day()`, similarly for
        // month/year). So once this chart's own comparison has fired once and
        // normalised the stored floor to the bucket boundary, any further floor
        // movement that stays inside that same bucket is invisible to it —
        // forever, not just for one cycle. The **daily** chart's own comparison
        // has no such blind spot (`Day::from_date` is the identity), so it is
        // used as the family's reliable detector: when it fires, every lower
        // resolution in the same family is forced to rebuild too, via a marker
        // left in `cx.cache` under the chart *name* (shared across resolutions,
        // unlike `ChartKey`).
        //
        // Reading a marker written by a *different* chart's `update_itself_inner`
        // call within this same cycle is safe only because dependants are
        // guaranteed to run after their dependencies: every lower-resolution
        // interchain chart takes the daily local-db chart as its main dependency
        // (`SumLowerResolution<MapParseTo<StripExt<…>>, …>`), and
        // `enabled_members_with_deps`/`update_recursively` always update a
        // chart's `MainDependencies` before the chart itself. So by the time a
        // weekly/monthly/yearly chart's `update_itself_inner` runs in a given
        // cycle, the daily chart in its family has already run in the same
        // cycle and already written (or not written) the marker for this
        // decision to read.
        let daily_sibling_marker = interchain_backfill_marker_statement::<ChartProps>();
        let daily_sibling_rebuilt = interchain_line_gate
            && ChartProps::resolution() != ResolutionKind::Day
            && cx
                .cache
                .get::<String>(&daily_sibling_marker)
                .await
                .is_some();
        let backfill_rebuild = own_floor_regressed.is_some() || daily_sibling_rebuilt;
        match &own_floor_regressed {
            Some((stored_floor, indexer_floor)) => {
                tracing::info!(
                    chart =% ChartProps::key(),
                    stored_floor =? stored_floor,
                    indexer_floor =? indexer_floor,
                    "interchain history floor regressed; recomputing the series from the \
                     indexer's current filtered floor (backfill pickup, no clear)"
                );
            }
            None if daily_sibling_rebuilt => {
                tracing::info!(
                    chart =% ChartProps::key(),
                    "the daily chart in this interchain family rebuilt from a floor \
                     regression this cycle; propagating the rebuild to this lower \
                     resolution, whose own bucket-level floor comparison cannot see an \
                     intra-bucket movement"
                );
            }
            None => {}
        }
        // Only the daily chart writes the marker — it is the only reliable
        // detector, and lower resolutions must not re-propagate a marker of
        // their own (there is nothing below them to propagate to, and doing so
        // would just be a no-op keyed the same way).
        if interchain_line_gate
            && ChartProps::resolution() == ResolutionKind::Day
            && backfill_rebuild
        {
            cx.cache.insert(&daily_sibling_marker, "1".to_owned()).await;
        }
        // NOTE: only the `force_full` *argument* below is affected — `cx` itself
        // (shared by every chart in the group) is never mutated. `cx.force_full`
        // has a second job a few lines above (gating the
        // `recorded_min_indexer_block` read), and conflating the two would change
        // read behaviour in non-interchain modes. The local override reuses
        // `Update::update_values`' existing "`last_accurate_point == None` ⇒
        // recompute from `get_min_date`" contract, and gives per-chart
        // granularity: within one group a base daily line may have regressed
        // while its cumulative dependant has not.
        let last_accurate_point = last_accurate_point::<ChartProps, Query>(
            min_indexer_block,
            recorded,
            cx.stats_db,
            cx.force_full || backfill_rebuild,
            ChartProps::approximate_trailing_points(),
            ChartProps::missing_date_policy(),
        )
        .await?;
        tracing::info!(last_accurate_point =? last_accurate_point, chart =% ChartProps::key(), "updating chart values");
        Update::update_values(
            cx,
            chart_id,
            last_accurate_point,
            min_indexer_block,
            dependency_data_fetch_timer,
        )
        .await?;
        tracing::info!(chart =% ChartProps::key(), "updating chart metadata");
        Update::update_metadata(cx.stats_db, chart_id, cx.time).await?;
        // Record whether this cycle's own trigger-2 rebuild (if any) actually
        // moved the chart's stored floor, so a byte-for-byte repeat of an
        // unproductive pair can be suppressed next cycle (see the suppression
        // comment above and `InterchainBackfillMemory`). Keyed on
        // `own_floor_regressed` (the post-suppression value) rather than
        // `raw_floor_regressed`: a cycle that got suppressed did not actually
        // rebuild from the floor this time, so it has nothing new to report
        // either way, and must leave the existing record alone. Likewise this
        // is never reached via `daily_sibling_rebuilt` alone — a chart that
        // only rebuilt because its daily sibling did had no floor regression
        // of its own to prove anything about, and must not overwrite (or
        // clear) whatever this chart's own memory currently holds.
        if let (Some((stored_floor, indexer_floor)), Some(memory)) =
            (&own_floor_regressed, cx.interchain_backfill_memory.as_ref())
        {
            let pair = (
                stored_floor.clone().into_date(),
                indexer_floor.clone().into_date(),
            );
            let chart_key = ChartProps::key();
            let new_stored_floor = recorded_min_chart_date(cx.stats_db, chart_id).await?;
            if new_stored_floor == Some(pair.0) {
                memory.record_unproductive(chart_key, pair).await;
            } else {
                // the floor moved (or, degenerately, the chart is now empty) —
                // any previously recorded pair no longer describes this
                // chart's situation and must not linger to suppress a future,
                // different regression
                memory.clear(&chart_key).await;
            }
        }
        Ok(())
    }

    fn observe_query_time(time: Duration) {
        if time > Duration::ZERO {
            metrics::CHART_FETCH_NEW_DATA_TIME
                .with_label_values(&[&ChartProps::key().to_string()])
                .observe(time.as_secs_f64());
        }
    }
}

/// Compare timestamps as they're seen in Postgres (compare up to microseconds)
fn postgres_timestamps_eq(time_1: DateTime<Utc>, time_2: DateTime<Utc>) -> bool {
    // PostgreSQL stores timestamps with microsecond precision
    // therefore, we need to drop any values smaller than microsecond
    // microsecond = 10^(-6) => compare up to 6 digits after comma
    time_1.trunc_subsecs(6).eq(&time_2.trunc_subsecs(6))
}

/// `Some((stored_floor, indexer_floor))` when this chart's stored history starts
/// **later** than the indexer's current filtered floor — i.e. the indexer has
/// backfilled below the chart's anchor, and the series must be recomputed from
/// the true floor rather than only moving forward.
///
/// Both floors go through `ChartProps::Resolution::from_date`, which is exactly
/// how `BatchUpdate` derives `update_range_start`, so the comparison is in the
/// units the rebuild will use — for weekly/monthly/yearly charts as much as for
/// daily ones. `get_min_date(cx)` is filter-scoped and memoised in
/// `UpdateContext::cache`, so it costs one query pair per group update rather
/// than one per chart.
async fn interchain_history_floor_regressed<ChartProps>(
    cx: &UpdateContext<'_>,
    chart_id: i32,
) -> Result<Option<(ChartProps::Resolution, ChartProps::Resolution)>, ChartError>
where
    ChartProps: ChartProperties + ?Sized,
    ChartProps::Resolution: Ord + Clone + Debug,
{
    // a fresh chart, or one the fingerprint gate just cleared, has no floor to
    // regress, and `BatchUpdate` already starts at `get_min_date` then
    let Some(stored_floor) = recorded_min_chart_date(cx.stats_db, chart_id).await? else {
        return Ok(None);
    };
    let stored_floor = ChartProps::Resolution::from_date(stored_floor);
    let indexer_floor = ChartProps::Resolution::from_date(
        get_min_date(cx)
            .await
            .map(|time| time.date())
            .map_err(ChartError::IndexerDB)?,
    );
    if stored_floor > indexer_floor {
        Ok(Some((stored_floor, indexer_floor)))
    } else {
        Ok(None)
    }
}

/// The `cx.cache` key used to propagate "the daily chart in this family fired
/// trigger 2 this cycle" to its weekly/monthly/yearly siblings.
///
/// Keyed by `ChartProps::name()`, not `ChartProps::key()`: the name is exactly
/// what every resolution of one chart family shares, and `UpdateCache` is keyed
/// by an arbitrary `Statement`, so a synthetic one (never executed) is used
/// purely as a namespaced string key. `db_backend` is irrelevant here — chosen
/// once (`Postgres`) for a stable, unexecuted key.
fn interchain_backfill_marker_statement<ChartProps>() -> Statement
where
    ChartProps: ChartProperties + ?Sized,
{
    Statement::from_string(
        DbBackend::Postgres,
        format!("interchain_backfill_marker::{}", ChartProps::name()),
    )
}

impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps> DataSource
    for LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource + Sync,
    ResolutionDep: DataSource + Sync,
    Create: CreateBehaviour + Sync,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution> + Sync,
    Query: QueryBehaviour + Sync,
    ChartProps: ChartProperties,
    ChartProps::Resolution: Ord + Clone + Debug + Send,
{
    type MainDependencies = MainDep;
    type ResolutionDependencies = ResolutionDep;
    type Output = Query::Output;

    fn chart_key() -> Option<ChartKey> {
        Some(ChartProps::key())
    }

    fn indexing_status_self_requirement() -> IndexingStatus {
        ChartProps::indexing_status_requirement()
    }

    async fn init_itself(db: &DatabaseConnection, init_time: &DateTime<Utc>) -> Result<(), DbErr> {
        Create::create(db, init_time).await
    }

    async fn update_itself(cx: &UpdateContext<'_>) -> Result<(), ChartError> {
        // set up metrics + write some logs

        let mut dependency_data_fetch_timer = AggregateTimer::new();
        let _update_timer = metrics::CHART_UPDATE_TIME
            .with_label_values(&[&ChartProps::key().to_string()])
            .start_timer();
        tracing::info!(chart =% ChartProps::key(), "started chart update");

        Self::update_itself_inner(cx, &mut dependency_data_fetch_timer)
            .await
            .inspect_err(|err| {
                metrics::UPDATE_ERRORS
                    .with_label_values(&[&ChartProps::key().to_string()])
                    .inc();
                tracing::error!(
                    chart =% ChartProps::key(),
                    "error during updating chart: {}",
                    err
                );
            })?;

        Self::observe_query_time(dependency_data_fetch_timer.total_time());
        tracing::info!(chart =% ChartProps::key(), "successfully updated chart");
        Ok(())
    }

    async fn set_next_update_from_itself(
        db: &DatabaseConnection,
        update_from: chrono::NaiveDate,
    ) -> Result<(), ChartError> {
        // make a proper separate table/column and use it
        // if this approach brings some problems
        let metadata = get_chart_metadata(db, &ChartProps::key()).await?;
        let update_from = day_start(&update_from);
        match metadata.last_updated_at {
            Some(current_last_updated_at) if update_from <= current_last_updated_at => {
                set_last_updated_at(metadata.id, db, update_from)
                    .await
                    .map_err(ChartError::StatsDB)?;
            }
            Some(current_last_updated_at) => {
                tracing::warn!(
                    "not setting `last_updated_at` because current value ({}) is less than requested ({})",
                    current_last_updated_at,
                    update_from
                )
            }
            None => {
                tracing::warn!(
                    "not setting `last_updated_at` because the chart have never updated before"
                )
            }
        }
        Ok(())
    }

    async fn query_data(
        cx: &UpdateContext<'_>,
        range: UniversalRange<DateTime<Utc>>,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<Self::Output, ChartError> {
        let _timer = dependency_data_fetch_timer.start_interval();
        // maybe add `fill_missing_dates` parameter to current function as well in the future
        // to get rid of "Note" in the `DataSource`'s method documentation
        Query::query_data(cx, range, None, false).await
    }
}

// need to delegate these traits for update groups to use

impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps> Named
    for LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource,
    ResolutionDep: DataSource,
    Create: CreateBehaviour,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution>,
    Query: QueryBehaviour,
    ChartProps: ChartProperties + Named,
{
    fn name() -> String {
        ChartProps::name()
    }
}

#[portrait::fill(portrait::delegate(ChartProps))]
impl<MainDep, ResolutionDep, Create, Update, Query, ChartProps> ChartProperties
    for LocalDbChartSource<MainDep, ResolutionDep, Create, Update, Query, ChartProps>
where
    MainDep: DataSource + Sync,
    ResolutionDep: DataSource + Sync,
    Create: CreateBehaviour + Sync,
    Update: UpdateBehaviour<MainDep, ResolutionDep, ChartProps::Resolution> + Sync,
    Query: QueryBehaviour + Sync,
    ChartProps: ChartProperties,
{
}

#[cfg(test)]
mod tests {
    use crate::{
        counters::TotalTxns,
        data_source::UpdateParameters,
        tests::{
            mock_blockscout::{fill_mock_blockscout_data, imitate_reindex},
            point_construction::d,
            simple_test::{get_counter, prepare_blockscout_chart_test},
        },
    };

    use super::*;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_total_txns_with_reindex() {
        let test_name = "update_total_txns_with_reindex";
        let (current_time, db, blockscout) =
            prepare_blockscout_chart_test::<TotalTxns>(test_name, None).await;
        let current_date = current_time.date_naive();
        fill_mock_blockscout_data(&blockscout, current_date).await;

        // Initial update and verify
        let parameters = UpdateParameters::default_test_parameters(
            &db,
            &blockscout,
            TotalTxns::all_dependencies_chart_keys(),
            Some(current_time),
        );

        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        TotalTxns::update_recursively(&cx).await.unwrap();
        assert_eq!("58", get_counter::<TotalTxns>(&cx).await.value);

        // Reindex blockscout data
        imitate_reindex(&blockscout, current_date).await;

        // Eight transactions were added as a result of reindex
        // `TotalTxns` calculates all data at once, so the date to update from
        // does not make a difference here.

        TotalTxns::set_next_update_from_recursively(&db, d("2023-01-02"))
            .await
            .unwrap();
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        TotalTxns::update_recursively(&cx).await.unwrap();
        assert_eq!("66", get_counter::<TotalTxns>(&cx).await.value);
    }

    mod update_itself_is_triggered_once_per_group {
        use std::{
            collections::HashSet,
            ops::DerefMut,
            str::FromStr,
            sync::{Arc, OnceLock},
        };

        use blockscout_metrics_tools::AggregateTimer;
        use chrono::{DateTime, Days, NaiveDate, TimeDelta, Utc};
        use entity::sea_orm_active_enums::ChartType;
        use tokio::sync::Mutex;

        use crate::{
            ChartError, ChartProperties, Named,
            charts::db_interaction::write::insert_data_many,
            construct_update_group,
            data_source::{
                DataSource, UpdateContext, UpdateParameters,
                kinds::local_db::{
                    DirectPointLocalDbChartSource, LocalDbChartSource,
                    parameter_traits::UpdateBehaviour,
                    parameters::{DefaultCreate, DefaultQueryLast},
                },
                types::Get,
            },
            gettable_const,
            tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data},
            types::{TimespanValue, timespans::DateValue},
            update_group::{SyncUpdateGroup, UpdateGroup},
        };

        type WasTriggeredStorage = Arc<Mutex<bool>>;

        // `OnceLock` in order to return the same instance each time
        static FLAG: OnceLock<WasTriggeredStorage> = OnceLock::new();

        gettable_const!(WasTriggered: WasTriggeredStorage = FLAG.get_or_init(|| Arc::new(Mutex::new(false))).clone());

        struct UpdateSingleTriggerAsserter;

        impl UpdateSingleTriggerAsserter {
            pub async fn record_trigger() {
                let mut was_triggered_guard = WasTriggered::get().lock_owned().await;
                let was_triggered = was_triggered_guard.deref_mut();
                assert!(!*was_triggered, "update triggered twice");
                *was_triggered = true;
            }

            pub async fn reset_triggers() {
                let mut was_triggered_guard = WasTriggered::get().lock_owned().await;
                let was_triggered = was_triggered_guard.deref_mut();
                *was_triggered = false;
            }
        }

        impl<M, R, Resolution> UpdateBehaviour<M, R, Resolution> for UpdateSingleTriggerAsserter
        where
            M: DataSource,
            R: DataSource,
            Resolution: Send,
        {
            async fn update_values(
                cx: &UpdateContext<'_>,
                chart_id: i32,
                _last_accurate_point: Option<TimespanValue<Resolution, String>>,
                min_indexer_block: i64,
                _dependency_data_fetch_timer: &mut AggregateTimer,
            ) -> Result<(), ChartError> {
                Self::record_trigger().await;
                // insert smth for dependency to work well
                let data = DateValue::<String> {
                    timespan: cx.time.date_naive(),
                    value: "0".to_owned(),
                };
                let value = data.active_model(chart_id, Some(min_indexer_block));
                insert_data_many(cx.stats_db, vec![value])
                    .await
                    .map_err(ChartError::StatsDB)?;
                Ok(())
            }
        }

        struct TestedChartProps;

        impl Named for TestedChartProps {
            fn name() -> String {
                "double_update_tested_chart".into()
            }
        }

        impl ChartProperties for TestedChartProps {
            type Resolution = NaiveDate;

            fn chart_type() -> ChartType {
                ChartType::Counter
            }
        }

        type TestedChart = LocalDbChartSource<
            (),
            (),
            DefaultCreate<TestedChartProps>,
            UpdateSingleTriggerAsserter,
            DefaultQueryLast<TestedChartProps>,
            TestedChartProps,
        >;

        struct ChartDependedOnTestedProps;

        impl Named for ChartDependedOnTestedProps {
            fn name() -> String {
                "double_update_dependant_chart".into()
            }
        }

        impl ChartProperties for ChartDependedOnTestedProps {
            type Resolution = NaiveDate;

            fn chart_type() -> ChartType {
                ChartType::Counter
            }
        }

        type ChartDependedOnTested =
            DirectPointLocalDbChartSource<TestedChart, ChartDependedOnTestedProps>;

        construct_update_group!(TestUpdateGroup {
            charts: [TestedChart, ChartDependedOnTested]
        });

        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn update_itself_is_triggered_once_per_group() {
            let _ = tracing_subscriber::fmt::try_init();
            let (db, blockscout) = init_db_all("update_itself_is_triggered_once_per_group").await;
            let current_time = DateTime::<Utc>::from_str("2023-03-01T12:00:00Z").unwrap();
            let current_date = current_time.date_naive();
            fill_mock_blockscout_data(&blockscout, current_date).await;
            let enabled = HashSet::from(
                [TestedChartProps::key(), ChartDependedOnTestedProps::key()].map(|l| l.to_owned()),
            );
            let mutexes = TestUpdateGroup
                .list_dependency_mutex_ids()
                .into_iter()
                .map(|id| (id.to_owned(), Arc::new(Mutex::new(()))))
                .collect();
            let group = SyncUpdateGroup::new(&mutexes, Arc::new(TestUpdateGroup)).unwrap();
            group
                .create_charts_sync(&db, Some(current_time), &enabled)
                .await
                .unwrap();

            let next_time = current_time.checked_add_days(Days::new(1)).unwrap();
            let parameters = UpdateParameters::default_test_parameters(
                &db,
                &blockscout,
                group.enabled_members_with_deps(&enabled),
                Some(next_time),
            )
            .with_force_full();
            group
                .update_charts_sync(parameters, &enabled)
                .await
                .unwrap();

            UpdateSingleTriggerAsserter::reset_triggers().await;

            let next_next_time = next_time.checked_add_days(Days::new(1)).unwrap();
            // it also works with high-precision timestamps
            //
            // regression: had a bug where due to postgres having resolution of 1 microsecond stored a different
            // timestamp to the one provided
            let time = next_next_time + TimeDelta::nanoseconds(1);
            let parameters = UpdateParameters::default_test_parameters(
                &db,
                &blockscout,
                group.enabled_members_with_deps(&enabled),
                Some(time),
            )
            .with_force_full();
            group
                .update_charts_sync(parameters, &enabled)
                .await
                .unwrap();

            UpdateSingleTriggerAsserter::reset_triggers().await;

            // also test if there is any rounding when inserting metadata
            let time = next_next_time + TimeDelta::nanoseconds(500);
            let parameters = UpdateParameters::default_test_parameters(
                &db,
                &blockscout,
                group.enabled_members_with_deps(&enabled),
                Some(time),
            )
            .with_force_full();
            group
                .update_charts_sync(parameters, &enabled)
                .await
                .unwrap();

            // also test if there is any rounding when inserting metadata
            let time = next_next_time + TimeDelta::nanoseconds(999);
            let parameters = UpdateParameters::default_test_parameters(
                &db,
                &blockscout,
                group.enabled_members_with_deps(&enabled),
                Some(time),
            )
            .with_force_full();
            group
                .update_charts_sync(parameters, &enabled)
                .await
                .unwrap();
        }
    }

    /// Reproduces, and verifies the fix for, the loop documented on
    /// [`super::super::super::types::InterchainBackfillMemory`]: a
    /// message-family interchain line chart whose own predicate never matches
    /// any row between the transfer-filtered floor and the message-filtered
    /// floor triggers trigger 2 (`interchain_history_floor_regressed`) every
    /// cycle, forever, because the rebuild it keeps causing can never move
    /// its own stored floor.
    ///
    /// The divergence between the two floors is the same one documented in
    /// `.memory-bank/gotchas.md` under "A Transfer's Token Chains Are Not Its
    /// Message's Route": a transfer can satisfy `transfers_condition()` while
    /// its own parent message fails `messages_condition()`. Filtering on
    /// `dst_chain_ids` reproduces it directly, since the same list is applied
    /// to `crosschain_messages.dst_chain_id` and to
    /// `crosschain_transfers.token_dst_chain_id` independently.
    mod interchain_backfill_floor_regression_suppression {
        use chrono::{DateTime, Days, NaiveDate, Utc};
        use interchain_indexer_filters::ChainBridgeFilter;
        use pretty_assertions::assert_eq;
        use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

        use crate::{
            ChartProperties, InterchainFilter, Mode,
            charts::{
                db_interaction::read::{find_chart, recorded_min_chart_date},
                lines::interchain::new_messages_interchain::{self, NewMessagesInterchain},
            },
            data_source::{
                DataSource, UpdateContext,
                types::{IndexerMigrations, InterchainBackfillMemory, UpdateParameters},
            },
            tests::{
                mock_interchain::test_interchain_filter,
                simple_test::prepare_interchain_chart_test_unfilled,
            },
        };

        use super::super::{
            interchain_backfill_marker_statement, interchain_history_floor_regressed,
        };

        const BRIDGE_ID: i32 = 1;
        const SRC_CHAIN: i64 = 10;
        /// A message ending here fails the test's `dst_chain_ids` filter.
        const OTHER_DST_CHAIN: i64 = 20;
        /// The filter's configured `dst_chain_ids` target.
        const TARGET_DST_CHAIN: i64 = 30;

        async fn exec(interchain: &DatabaseConnection, sql: String) {
            interchain
                .execute(Statement::from_string(DbBackend::Postgres, sql))
                .await
                .unwrap();
        }

        async fn insert_reference_rows(interchain: &DatabaseConnection) {
            for chain_id in [SRC_CHAIN, OTHER_DST_CHAIN, TARGET_DST_CHAIN] {
                exec(
                    interchain,
                    format!(
                        "INSERT INTO chains (id, name) VALUES ({chain_id}, 'chain_{chain_id}')"
                    ),
                )
                .await;
            }
            exec(
                interchain,
                format!(
                    "INSERT INTO bridges (id, name) VALUES ({BRIDGE_ID}, 'bridge_{BRIDGE_ID}')"
                ),
            )
            .await;
        }

        /// A message whose own route matches the filter (`dst = TARGET_DST_CHAIN`):
        /// the message-family chart actually stores a row for it.
        async fn insert_matching_message(
            interchain: &DatabaseConnection,
            id: i64,
            init_timestamp: &str,
        ) {
            insert_message_and_transfer(interchain, id, init_timestamp, TARGET_DST_CHAIN).await;
        }

        /// A message whose own route does NOT match the filter (`dst =
        /// OTHER_DST_CHAIN`), paired with a transfer whose own token route DOES
        /// (`token_dst_chain_id = TARGET_DST_CHAIN`) — the "transfer's token
        /// chains are not its message's route" case. Passes
        /// `transfers_condition()`, fails `messages_condition()`.
        async fn insert_diverging_message(
            interchain: &DatabaseConnection,
            id: i64,
            init_timestamp: &str,
        ) {
            insert_message_and_transfer(interchain, id, init_timestamp, OTHER_DST_CHAIN).await;
        }

        /// Inserts one message (route `SRC_CHAIN -> message_dst_chain`) and its
        /// one transfer, whose token route is always `SRC_CHAIN ->
        /// TARGET_DST_CHAIN` regardless of `message_dst_chain` — the token route
        /// deliberately does not follow the message's own route, matching the
        /// gotcha this test reproduces.
        async fn insert_message_and_transfer(
            interchain: &DatabaseConnection,
            id: i64,
            init_timestamp: &str,
            message_dst_chain: i64,
        ) {
            exec(
                interchain,
                format!(
                    "INSERT INTO crosschain_messages \
                     (id, bridge_id, init_timestamp, src_chain_id, dst_chain_id) \
                     VALUES ({id}, {BRIDGE_ID}, '{init_timestamp}', {SRC_CHAIN}, {message_dst_chain})"
                ),
            )
            .await;
            exec(
                interchain,
                format!(
                    "INSERT INTO crosschain_transfers \
                     (id, message_id, bridge_id, index, token_src_chain_id, token_dst_chain_id) \
                     VALUES ({id}, {id}, {BRIDGE_ID}, 0, {SRC_CHAIN}, {TARGET_DST_CHAIN})"
                ),
            )
            .await;
        }

        fn filter() -> InterchainFilter {
            test_interchain_filter(ChainBridgeFilter {
                dst_chain_ids: Some(vec![TARGET_DST_CHAIN]),
                ..Default::default()
            })
        }

        fn parameters<'a>(
            stats_db: &'a DatabaseConnection,
            indexer_db: &'a DatabaseConnection,
            update_time: DateTime<Utc>,
            memory: InterchainBackfillMemory,
        ) -> UpdateParameters<'a> {
            UpdateParameters {
                stats_db,
                mode: Mode::Interchain,
                multichain_filter: None,
                interchain_filter: filter(),
                indexer_db,
                indexer_applied_migrations: IndexerMigrations::latest(),
                second_indexer_db: None,
                enabled_update_charts_recursive: NewMessagesInterchain::all_dependencies_chart_keys(
                ),
                update_time_override: Some(update_time),
                force_full: false,
                interchain_backfill_memory: None,
            }
            .with_interchain_backfill_memory(memory)
        }

        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn suppresses_repeat_unproductive_trigger_but_not_a_genuine_further_regression() {
            let _ = tracing_subscriber::fmt::try_init();
            let (init_time, db, indexer) =
                prepare_interchain_chart_test_unfilled::<NewMessagesInterchain>(
                    "interchain_backfill_suppression_reproduces_loop",
                )
                .await;
            insert_reference_rows(&indexer).await;
            // only the matching message exists so far — the chart's initial
            // build sees a floor that agrees with the (not yet backfilled)
            // indexer floor, so trigger 2 has nothing to fire on yet
            insert_matching_message(&indexer, 1, "2023-01-10 10:00:00").await;

            let chart_id = find_chart(&db, &new_messages_interchain::Properties::key())
                .await
                .unwrap()
                .expect("chart row must exist after init_recursively");
            let floor_10th = NaiveDate::from_ymd_opt(2023, 1, 10).unwrap();
            let floor_1st = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
            let floor_dec_1st = NaiveDate::from_ymd_opt(2022, 12, 1).unwrap();

            let memory = InterchainBackfillMemory::new();

            // cycle 0: initial build.
            let cx0 = UpdateContext::from_params_now_or_override(parameters(
                &db,
                &indexer,
                init_time,
                memory.clone(),
            ));
            NewMessagesInterchain::update_recursively(&cx0)
                .await
                .unwrap();
            assert_eq!(
                recorded_min_chart_date(&db, chart_id).await.unwrap(),
                Some(floor_10th),
                "the fresh chart's floor is the only message that passes the filter"
            );

            // the indexer backfills an earlier message whose OWN route fails the
            // filter, but whose transfer's token route matches it — this pulls
            // the shared indexer floor below the chart's stored floor without
            // ever giving the chart new data to store there
            insert_diverging_message(&indexer, 2, "2023-01-01 10:00:00").await;

            let update_time_1 = init_time.checked_add_days(Days::new(1)).unwrap();
            let cx1 = UpdateContext::from_params_now_or_override(parameters(
                &db,
                &indexer,
                update_time_1,
                memory.clone(),
            ));
            let raw_1 = interchain_history_floor_regressed::<new_messages_interchain::Properties>(
                &cx1, chart_id,
            )
            .await
            .unwrap();
            assert_eq!(
                raw_1,
                Some((floor_10th, floor_1st)),
                "the stored floor sits above the shared, filter-scoped indexer floor"
            );

            NewMessagesInterchain::update_recursively(&cx1)
                .await
                .unwrap();
            // the marker is the daily chart's own reliable "did trigger 2 fire
            // this cycle" signal (only the daily chart writes it, and only when
            // its own `backfill_rebuild` is true) — see the comments in
            // `update_itself_inner`
            assert_eq!(
                cx1.cache
                    .get::<String>(&interchain_backfill_marker_statement::<
                        new_messages_interchain::Properties,
                    >())
                    .await,
                Some("1".to_owned()),
                "the first cycle under the new (lower) indexer floor must trigger a rebuild"
            );
            assert_eq!(
                recorded_min_chart_date(&db, chart_id).await.unwrap(),
                Some(floor_10th),
                "the rebuild is unproductive: the diverging message still fails the filter, so \
                 nothing gets inserted below the chart's existing floor"
            );
            assert!(
                memory
                    .is_known_unproductive(
                        &new_messages_interchain::Properties::key(),
                        (floor_10th, floor_1st)
                    )
                    .await,
                "the fix must remember this exact pair as unproductive after the rebuild"
            );

            // cycle 2: same filter, same floors — the loop's second iteration.
            // Without suppression this triggers (and rebuilds) identically to
            // cycle 1, forever.
            let update_time_2 = update_time_1.checked_add_days(Days::new(1)).unwrap();
            let cx2 = UpdateContext::from_params_now_or_override(parameters(
                &db,
                &indexer,
                update_time_2,
                memory.clone(),
            ));
            let raw_2 = interchain_history_floor_regressed::<new_messages_interchain::Properties>(
                &cx2, chart_id,
            )
            .await
            .unwrap();
            assert_eq!(
                raw_2, raw_1,
                "the underlying condition is unchanged — this is a genuine repeat, not a \
                 regression that resolved itself on its own"
            );

            NewMessagesInterchain::update_recursively(&cx2)
                .await
                .unwrap();
            assert_eq!(
                cx2.cache
                    .get::<String>(&interchain_backfill_marker_statement::<
                        new_messages_interchain::Properties,
                    >())
                    .await,
                None,
                "the second cycle under the identical, already-proven-unproductive pair must \
                 NOT trigger another rebuild — this is what fails against the pre-fix code"
            );

            // a genuine further regression — the indexer floor moving even
            // lower — must still trigger, because it is a different pair the
            // memory has never proven unproductive
            insert_diverging_message(&indexer, 3, "2022-12-01 10:00:00").await;
            let update_time_3 = update_time_2.checked_add_days(Days::new(1)).unwrap();
            let cx3 = UpdateContext::from_params_now_or_override(parameters(
                &db,
                &indexer,
                update_time_3,
                memory.clone(),
            ));
            let raw_3 = interchain_history_floor_regressed::<new_messages_interchain::Properties>(
                &cx3, chart_id,
            )
            .await
            .unwrap();
            assert_eq!(
                raw_3,
                Some((floor_10th, floor_dec_1st)),
                "the indexer floor moved further down"
            );
            NewMessagesInterchain::update_recursively(&cx3)
                .await
                .unwrap();
            assert_eq!(
                cx3.cache
                    .get::<String>(&interchain_backfill_marker_statement::<
                        new_messages_interchain::Properties,
                    >())
                    .await,
                Some("1".to_owned()),
                "a genuinely new (lower) indexer floor must trigger again even though the \
                 previous pair was suppressed"
            );
        }
    }
}
