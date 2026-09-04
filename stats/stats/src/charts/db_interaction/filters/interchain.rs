// SPDX-License-Identifier: LicenseRef-Blockscout

//! The interchain read filter.
//!
//! The predicate itself is NOT defined here. It lives in the
//! `interchain-indexer-filters` crate, shared verbatim with
//! `interchain-indexer`'s read API, so that "stats and the API return the same
//! subset" is a property of the build graph rather than a claim a reviewer
//! re-checks. If the predicate needs to change, change it there and review the
//! change against both services.
//!
//! What lives here is stats-specific: the wrapper that update contexts carry,
//! the three sanctioned query entry points, and the fingerprint that detects a
//! configuration change against already-stored chart data.

use std::collections::BTreeSet;

use interchain_indexer_entity::{crosschain_messages, crosschain_transfers};
use interchain_indexer_filters::ChainBridgeFilter;
use sea_orm::{
    Condition, DbBackend, EntityTrait, JoinType, QueryFilter, QuerySelect, QueryTrait,
    RelationTrait, Select, Statement,
};

/// The interchain read filter as the update pipeline sees it.
///
/// `condition_source` is fully resolved: the operator-configured dimensions plus
/// the observability horizon read from the indexer DB for this update cycle.
/// `fingerprint` is deliberately NOT derived from it — see [`filter_fingerprint`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterchainFilter {
    pub condition_source: ChainBridgeFilter,
    pub fingerprint: i64,
}

impl InterchainFilter {
    /// No filter at all — every predicate absent.
    ///
    /// Note the fingerprint is a real hash, not `0`: a deployment that goes from
    /// filtered to unfiltered must still be detected as a change.
    pub fn unfiltered() -> Self {
        let condition_source = ChainBridgeFilter::default();
        Self {
            fingerprint: filter_fingerprint(&condition_source, false),
            condition_source,
        }
    }

    pub fn messages_condition(&self) -> Condition {
        self.condition_source.messages_condition()
    }

    pub fn transfers_condition(&self) -> Condition {
        self.condition_source.transfers_condition()
    }

    pub fn home_chain_id(&self) -> Option<i64> {
        self.condition_source.home_chain_id
    }

    pub fn is_empty(&self) -> bool {
        self.condition_source.is_empty()
    }

    /// Start a `crosschain_messages` query with the filter already applied.
    /// Interchain statements MUST start from one of these three methods and must
    /// never call `crosschain_messages::Entity::find()` directly.
    pub fn messages_query(&self) -> Select<crosschain_messages::Entity> {
        crosschain_messages::Entity::find().filter(self.messages_condition())
    }

    /// Start a `crosschain_transfers` query with the filter already applied.
    /// No join: [`Self::transfers_condition`] touches only the transfer's own
    /// columns.
    pub fn transfers_query(&self) -> Select<crosschain_transfers::Entity> {
        crosschain_transfers::Entity::find().filter(self.transfers_condition())
    }

    /// As [`Self::transfers_query`], plus the composite join to the parent
    /// message. The join exists ONLY to reach `crosschain_messages.init_timestamp`
    /// (transfers have no timestamp of their own) and `src_tx_hash`/`dst_tx_hash`.
    /// The filter stays on the transfer's own columns. The declared relation
    /// emits both halves of `(message_id, bridge_id) = (id, bridge_id)`; never
    /// hand-write this join.
    pub fn transfers_joined_query(&self) -> Select<crosschain_transfers::Entity> {
        self.transfers_query().join(
            JoinType::InnerJoin,
            crosschain_transfers::Relation::CrosschainMessages.def(),
        )
    }
}

impl Default for InterchainFilter {
    fn default() -> Self {
        Self::unfiltered()
    }
}

/// Which of the two predicates an interchain statement is required to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterchainFilterTarget {
    Messages,
    Transfers,
}

/// Implemented by every interchain remote statement, so a test can prove the
/// filter is applied — and applied exactly as often as declared.
///
/// This is the coverage guard, and it is deliberately not a source grep. Four of
/// fifteen chart families were unfiltered before this change precisely because
/// coverage was procedural. See [`crate::charts::interchain_filter_coverage`]
/// for the registry and the two tests that consume this trait, including an
/// honest statement of what they do and do not enforce.
pub trait InterchainFiltered {
    /// Which of the two predicates this statement is required to apply.
    const TARGET: InterchainFilterTarget;
    /// How many times the predicate appears in the rendered SQL. `1` for twelve
    /// of the thirteen; `2` for `totalInterchainTransferUsers`, whose UNION has
    /// two arms over the same table.
    const EXPECTED_APPLICATIONS: usize = 1;
    /// The public chart id this statement serves.
    const CHART_NAME: &'static str;
    /// Render with an explicit filter, so the test needs no `UpdateContext` and
    /// therefore no database connections. Line charts pass `None` for the range.
    fn render(filter: &InterchainFilter) -> Statement;
}

/// The chain ids a row admitted by `configured` can possibly name — an **upper
/// bound**, not an exact set. `None` = unbounded (every chain).
///
/// Used to scope the catch-up verdict: an indexing pair `(b, c)` can only create
/// or update a row whose `bridge_id = b` and one of whose endpoints is `c`, so a
/// pair whose chain falls outside this bound cannot influence the configured
/// slice and must not delay a recompute.
///
/// Projects onto the **message** route. Exact for the 4 message chart families;
/// an accepted assumption for the 3 transfer families, which filter on the
/// transfer's own `token_src_chain_id` / `token_dst_chain_id` — see
/// `.memory-bank/gotchas.md` → "A Transfer's Token Chains Are Not Its Message's
/// Route".
///
/// Deliberately ignores `only_indexed_by_bridge`: the horizon is DB-derived and
/// per-cycle, and the failure direction here prefers a **wide** bound (an
/// over-narrow scope produces a false "synced" and silently wrong data; an
/// over-wide one only produces extra work).
pub fn relevant_chain_ids(configured: &ChainBridgeFilter) -> Option<BTreeSet<i64>> {
    let mut bound: Option<BTreeSet<i64>> = None;

    let mut intersect = |ids: BTreeSet<i64>| {
        bound = Some(match bound.take() {
            Some(existing) => existing.intersection(&ids).cloned().collect(),
            None => ids,
        });
    };

    match (
        configured.home_chain_id,
        configured.counterparty_chain_ids.as_deref(),
    ) {
        (Some(home), Some(counterparties)) => {
            let mut ids: BTreeSet<i64> = counterparties.iter().cloned().collect();
            ids.insert(home);
            intersect(ids);
        }
        (Some(_), None) => {
            // focal alone: `src = home OR dst = home` leaves the other side open
        }
        (None, Some(counterparties)) => {
            intersect(counterparties.iter().cloned().collect());
        }
        (None, None) => {}
    }

    // exactly one side bounded leaves the other side open
    if let (Some(src), Some(dst)) = (
        configured.src_chain_ids.as_deref(),
        configured.dst_chain_ids.as_deref(),
    ) {
        let mut ids: BTreeSet<i64> = src.iter().cloned().collect();
        ids.extend(dst.iter().cloned());
        intersect(ids);
    }

    bound
}

/// The operator-configured half of the filter, resolved from settings once at
/// startup. The observability horizon is added per update cycle by
/// [`Self::with_horizon`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterchainFilterConfig {
    /// `only_indexed_by_bridge` is always `None` here.
    configured: ChainBridgeFilter,
    include_unindexed_chains: bool,
    fingerprint: i64,
}

impl InterchainFilterConfig {
    pub fn new(configured: ChainBridgeFilter, include_unindexed_chains: bool) -> Self {
        // `assert!`, not `debug_assert!`: this runs once, at startup, so the check
        // costs nothing, and both consequences of violating it are silent in a
        // release build. `with_horizon` would overwrite the field, dropping a
        // caller-supplied horizon so the restriction never applies; and
        // `filter_fingerprint` would hash the horizon's *contents*, so every
        // upstream bridge addition would clear and rebuild every interchain chart —
        // the outcome the fingerprint is explicitly designed to avoid.
        assert!(
            configured.only_indexed_by_bridge.is_none(),
            "the horizon is resolved per update cycle, not carried in the config"
        );
        let fingerprint = filter_fingerprint(&configured, !include_unindexed_chains);
        Self {
            configured,
            include_unindexed_chains,
            fingerprint,
        }
    }

    /// No filter at all: nothing configured, and the horizon restriction
    /// disabled. Equivalent to [`InterchainFilter::unfiltered`] after
    /// [`Self::with_horizon`] — including the fingerprint.
    pub fn unfiltered() -> Self {
        Self::new(ChainBridgeFilter::default(), true)
    }

    pub fn include_unindexed_chains(&self) -> bool {
        self.include_unindexed_chains
    }

    pub fn bridge_ids(&self) -> Option<&[i32]> {
        self.configured.bridge_ids.as_deref()
    }

    /// See [`relevant_chain_ids`]. The **operator-configured** half only.
    pub fn relevant_chain_ids(&self) -> Option<BTreeSet<i64>> {
        relevant_chain_ids(&self.configured)
    }

    /// Merge the horizon resolved for this update cycle. The fingerprint is
    /// carried through unchanged.
    pub fn with_horizon(
        &self,
        only_indexed_by_bridge: Option<Vec<(i32, Vec<i64>)>>,
    ) -> InterchainFilter {
        InterchainFilter {
            condition_source: ChainBridgeFilter {
                only_indexed_by_bridge,
                ..self.configured.clone()
            },
            fingerprint: self.fingerprint,
        }
    }

    /// The **operator-configured** half of the filter, rendered as a SQL `WHERE`
    /// clause for the startup log.
    ///
    /// Not the effective predicate: the horizon is rendered as `None` because it
    /// is not known until the first update cycle reads it from the indexer DB.
    /// With `include_unindexed_chains = false` — the default — the applied
    /// predicate is therefore strictly narrower than this, and an otherwise
    /// unconfigured filter renders `<unfiltered>` while still restricting rows.
    /// The caller must not describe this as what the service will count.
    pub fn render_for_log(&self) -> String {
        let filter = self.with_horizon(None);
        if filter.is_empty() {
            return "<unfiltered>".to_owned();
        }
        let sql = filter
            .messages_query()
            .build(DbBackend::Postgres)
            .to_string();
        sql.split_once(" WHERE ")
            .map(|(_, where_clause)| where_clause.to_owned())
            .unwrap_or(sql)
    }
}

impl Default for InterchainFilterConfig {
    fn default() -> Self {
        Self::unfiltered()
    }
}

/// Stable 63-bit hash of the *operator-configured* filter dimensions.
///
/// Deliberately NOT `std::hash::DefaultHasher`: std's hash outputs carry no
/// cross-version stability guarantee, and this value is persisted in
/// `chart_data.min_blockscout_block` and compared on the next run.
///
/// `configured.only_indexed_by_bridge` is `None` by construction, and the
/// horizon's *contents* are excluded on purpose: they are DB-derived and grow on
/// their own as bridges and contracts appear upstream, with no stats config
/// change. Hashing them would make every upstream bridge addition wipe and
/// rebuild all 39 interchain charts. Whether the restriction is *enabled*
/// (`horizon_enabled`) is a configuration statement and IS hashed — without it,
/// flipping `include_unindexed_chains` would leave the fingerprint unchanged
/// while moving every number.
///
/// The result is always a strictly positive `BIGINT` and never `i64::MAX` — the
/// constant every interchain row carries today (`get_min_block_interchain`). The
/// first update after this change therefore always detects a mismatch and
/// rebuilds, which is intended.
///
/// Note that masking to 63 bits alone does *not* give that: `0x7fff_ffff_ffff_ffff`
/// **is** `i64::MAX`, so the mask makes the one forbidden value reachable rather
/// than unreachable (and `0` reachable too). Both are remapped explicitly below,
/// so the guarantee holds for every input rather than for the ones a test
/// happens to sample.
pub fn filter_fingerprint(configured: &ChainBridgeFilter, horizon_enabled: bool) -> i64 {
    /// Feeding a version tag first lets a future change to this encoding force a
    /// deliberate rebuild.
    const VERSION_TAG: &[u8] = b"interchain-filter-v1";

    let mut hasher = Fnv1a::new();
    hasher.write(VERSION_TAG);
    // The presence byte AND the element count are both required: without them
    // `None` / `Some([])` / `Some([1, 2])` / `Some([12])` could collide.
    hasher.write_optional_i64(configured.home_chain_id);
    for list in [
        &configured.counterparty_chain_ids,
        &configured.src_chain_ids,
        &configured.dst_chain_ids,
    ] {
        hasher.write_optional_list(list.as_deref(), |h, id| h.write(&id.to_le_bytes()));
    }
    hasher.write_optional_list(configured.bridge_ids.as_deref(), |h, id| {
        h.write(&id.to_le_bytes())
    });
    hasher.write(&[horizon_enabled as u8]);
    match (hasher.finish() & 0x7fff_ffff_ffff_ffff) as i64 {
        // 2 of 2^63 outputs, but they are the two that would break the callers:
        // `0` is falsy-looking in a nullable BIGINT column, and `i64::MAX` is the
        // sentinel already stored on every interchain row.
        0 | i64::MAX => 1,
        fingerprint => fingerprint,
    }
}

/// FNV-1a 64, spelled out rather than pulled in as a dependency: the whole
/// algorithm is three lines and the *stability* of the output is the point.
struct Fnv1a(u64);

impl Fnv1a {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    fn new() -> Self {
        Self(Self::OFFSET_BASIS)
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn write_optional_i64(&mut self, value: Option<i64>) {
        match value {
            Some(value) => {
                self.write(&[1]);
                self.write(&value.to_le_bytes());
            }
            None => self.write(&[0]),
        }
    }

    fn write_optional_list<T>(
        &mut self,
        list: Option<&[T]>,
        write_element: impl Fn(&mut Self, &T),
    ) {
        match list {
            Some(elements) => {
                self.write(&[1]);
                self.write(&(elements.len() as u32).to_le_bytes());
                for element in elements {
                    write_element(self, element);
                }
            }
            None => self.write(&[0]),
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use sea_orm::QueryTrait;

    use super::*;
    use crate::tests::normalize_sql;

    fn configured(
        home_chain_id: Option<i64>,
        counterparty_chain_ids: Option<Vec<i64>>,
        src_chain_ids: Option<Vec<i64>>,
        dst_chain_ids: Option<Vec<i64>>,
        bridge_ids: Option<Vec<i32>>,
    ) -> ChainBridgeFilter {
        ChainBridgeFilter {
            home_chain_id,
            counterparty_chain_ids,
            src_chain_ids,
            dst_chain_ids,
            bridge_ids,
            only_indexed_by_bridge: None,
        }
    }

    fn chains(ids: &[i64]) -> BTreeSet<i64> {
        ids.iter().cloned().collect()
    }

    #[test]
    fn relevant_chains_unbounded_without_configured_dimensions() {
        assert_eq!(relevant_chain_ids(&ChainBridgeFilter::default()), None);
    }

    #[test]
    fn relevant_chains_focal_with_counterparties_is_home_plus_counterparties() {
        let filter = configured(Some(1), Some(vec![2, 3]), None, None, None);
        assert_eq!(relevant_chain_ids(&filter), Some(chains(&[1, 2, 3])));
    }

    #[test]
    fn relevant_chains_focal_alone_is_unbounded() {
        let filter = configured(Some(1), None, None, None, None);
        assert_eq!(relevant_chain_ids(&filter), None);
    }

    #[test]
    fn relevant_chains_counterparties_alone_is_the_within_set() {
        let filter = configured(None, Some(vec![2, 3]), None, None, None);
        assert_eq!(relevant_chain_ids(&filter), Some(chains(&[2, 3])));
    }

    #[test]
    fn relevant_chains_directional_requires_both_sides_to_bound() {
        // src alone: dst is open, so unbounded
        let src_only = configured(None, None, Some(vec![1]), None, None);
        assert_eq!(relevant_chain_ids(&src_only), None);

        // dst alone: src is open, so unbounded
        let dst_only = configured(None, None, None, Some(vec![1]), None);
        assert_eq!(relevant_chain_ids(&dst_only), None);

        // both sides: union of the two
        let both = configured(None, None, Some(vec![1]), Some(vec![2, 3]), None);
        assert_eq!(relevant_chain_ids(&both), Some(chains(&[1, 2, 3])));
    }

    #[test]
    fn relevant_chains_composes_bounds_by_intersection() {
        // focal (home + counterparties) ∩ directional (src ∪ dst)
        let filter = configured(
            Some(1),
            Some(vec![2, 3]),
            Some(vec![1, 2]),
            Some(vec![3, 4]),
            None,
        );
        // focal bound: {1, 2, 3}; directional bound: {1, 2, 3, 4}
        // intersection: {1, 2, 3}
        assert_eq!(relevant_chain_ids(&filter), Some(chains(&[1, 2, 3])));
    }

    #[test]
    fn relevant_chains_ignores_the_observability_horizon() {
        let filter = ChainBridgeFilter {
            only_indexed_by_bridge: Some(vec![(7, vec![99])]),
            ..configured(Some(1), None, None, None, None)
        };
        // still unbounded: home alone leaves the other side open, and the
        // horizon must not narrow it
        assert_eq!(relevant_chain_ids(&filter), None);
    }

    #[test]
    fn fingerprint_is_stable_across_calls() {
        let filter = configured(Some(1), Some(vec![2, 3]), None, None, Some(vec![7]));
        assert_eq!(
            filter_fingerprint(&filter, false),
            filter_fingerprint(&filter, false)
        );
    }

    #[test]
    fn fingerprint_is_always_a_positive_bigint() {
        let candidates = [
            ChainBridgeFilter::default(),
            configured(Some(i64::MAX), None, None, None, None),
            configured(Some(i64::MIN), None, None, None, None),
            configured(
                Some(-1),
                Some(vec![i64::MIN, i64::MAX]),
                Some(vec![0]),
                Some(vec![i64::MAX]),
                Some(vec![i32::MIN, i32::MAX]),
            ),
        ];
        for filter in candidates {
            for horizon_enabled in [false, true] {
                let fingerprint = filter_fingerprint(&filter, horizon_enabled);
                assert!(fingerprint > 0, "not positive: {fingerprint} ({filter:?})");
                assert_ne!(fingerprint, i64::MAX, "collides with today's constant");
            }
        }
    }

    #[test]
    fn fingerprint_changes_with_every_dimension() {
        let base = ChainBridgeFilter::default();
        let variants = [
            ("home_chain_id", configured(Some(1), None, None, None, None)),
            (
                "counterparty_chain_ids",
                configured(None, Some(vec![1]), None, None, None),
            ),
            (
                "src_chain_ids",
                configured(None, None, Some(vec![1]), None, None),
            ),
            (
                "dst_chain_ids",
                configured(None, None, None, Some(vec![1]), None),
            ),
            (
                "bridge_ids",
                configured(None, None, None, None, Some(vec![1])),
            ),
        ];
        let base_fingerprint = filter_fingerprint(&base, false);
        let mut seen = vec![base_fingerprint];
        for (dimension, variant) in variants {
            let fingerprint = filter_fingerprint(&variant, false);
            assert!(
                !seen.contains(&fingerprint),
                "{dimension} did not change the fingerprint"
            );
            seen.push(fingerprint);
        }
        // the sixth dimension is not a `ChainBridgeFilter` field
        let horizon_flipped = filter_fingerprint(&base, true);
        assert!(
            !seen.contains(&horizon_flipped),
            "flipping include_unindexed_chains did not change the fingerprint"
        );
    }

    #[test]
    fn fingerprint_distinguishes_none_empty_and_element_grouping() {
        let variants = [
            None,
            Some(vec![]),
            Some(vec![1, 2]),
            Some(vec![12]),
            Some(vec![2, 1]),
        ];
        let fingerprints: Vec<i64> = variants
            .iter()
            .map(|list| {
                filter_fingerprint(&configured(None, list.clone(), None, None, None), false)
            })
            .collect();
        for (i, left) in fingerprints.iter().enumerate() {
            for (j, right) in fingerprints.iter().enumerate() {
                if i != j {
                    assert_ne!(
                        left, right,
                        "{:?} and {:?} collide",
                        variants[i], variants[j]
                    );
                }
            }
        }
    }

    #[test]
    fn unfiltered_fingerprint_matches_the_default_configuration_without_horizon() {
        assert_eq!(
            InterchainFilter::default().fingerprint,
            filter_fingerprint(&ChainBridgeFilter::default(), false)
        );
        assert_eq!(
            InterchainFilterConfig::unfiltered()
                .with_horizon(None)
                .fingerprint,
            filter_fingerprint(&ChainBridgeFilter::default(), false)
        );
    }

    #[test]
    fn config_new_hashes_the_horizon_flag_inverted() {
        let filter = configured(Some(1), None, None, None, None);
        assert_eq!(
            InterchainFilterConfig::new(filter.clone(), false).fingerprint,
            filter_fingerprint(&filter, true)
        );
        assert_eq!(
            InterchainFilterConfig::new(filter.clone(), true).fingerprint,
            filter_fingerprint(&filter, false)
        );
    }

    #[test]
    fn with_horizon_keeps_the_configured_dimensions_and_the_fingerprint() {
        let config = InterchainFilterConfig::new(
            configured(Some(1), None, None, None, Some(vec![7])),
            false,
        );
        let filter = config.with_horizon(Some(vec![(7, vec![1, 2])]));
        assert_eq!(filter.home_chain_id(), Some(1));
        assert_eq!(
            filter.condition_source.bridge_ids.as_deref(),
            Some(&[7][..])
        );
        assert_eq!(
            filter.condition_source.only_indexed_by_bridge,
            Some(vec![(7, vec![1, 2])])
        );
        assert_eq!(filter.fingerprint, config.fingerprint);
    }

    fn where_clause(sql: &str) -> String {
        normalize_sql(
            sql.split_once(" WHERE ")
                .map(|(_, where_clause)| where_clause)
                .unwrap_or(""),
        )
    }

    #[test]
    fn entry_points_apply_the_predicate() {
        let filter = InterchainFilterConfig::new(
            configured(Some(1), Some(vec![2, 3]), None, None, Some(vec![7])),
            false,
        )
        .with_horizon(Some(vec![(7, vec![1, 2, 3])]));

        let messages = where_clause(
            &filter
                .messages_query()
                .build(DbBackend::Postgres)
                .to_string(),
        );
        assert!(
            messages.contains("\"src_chain_id\" = 1")
                && messages.contains("\"dst_chain_id\" IN (2, 3)")
                && messages.contains("\"bridge_id\" IN (7)"),
            "messages_query lost the predicate: {messages}"
        );

        for (name, sql) in [
            (
                "transfers_query",
                filter
                    .transfers_query()
                    .build(DbBackend::Postgres)
                    .to_string(),
            ),
            (
                "transfers_joined_query",
                filter
                    .transfers_joined_query()
                    .build(DbBackend::Postgres)
                    .to_string(),
            ),
        ] {
            let transfers = where_clause(&sql);
            assert!(
                transfers.contains("\"token_src_chain_id\" = 1")
                    && transfers.contains("\"token_dst_chain_id\" IN (2, 3)")
                    && transfers.contains("\"bridge_id\" IN (7)"),
                "{name} lost the predicate: {transfers}"
            );
        }
    }

    #[test]
    fn transfers_joined_query_renders_both_halves_of_the_composite_join() {
        let sql = normalize_sql(
            &InterchainFilter::default()
                .transfers_joined_query()
                .build(DbBackend::Postgres)
                .to_string(),
        );
        assert!(
            sql.contains(
                r#"INNER JOIN "crosschain_messages" ON "crosschain_transfers"."message_id" = "crosschain_messages"."id" AND "crosschain_transfers"."bridge_id" = "crosschain_messages"."bridge_id""#
            ),
            "composite join not rendered in full: {sql}"
        );
    }

    #[test]
    fn default_filter_adds_no_chain_or_bridge_term() {
        for (name, sql) in [
            (
                "messages_query",
                InterchainFilter::default()
                    .messages_query()
                    .build(DbBackend::Postgres)
                    .to_string(),
            ),
            (
                "transfers_query",
                InterchainFilter::default()
                    .transfers_query()
                    .build(DbBackend::Postgres)
                    .to_string(),
            ),
        ] {
            let where_clause = where_clause(&sql).to_ascii_lowercase();
            assert!(
                !where_clause.contains("chain_id") && !where_clause.contains("bridge_id"),
                "{name} added a predicate for the unfiltered case: {sql}"
            );
        }
    }

    #[test]
    fn render_for_log_reports_the_where_clause_or_unfiltered() {
        assert_eq!(
            InterchainFilterConfig::unfiltered().render_for_log(),
            "<unfiltered>"
        );
        let rendered = InterchainFilterConfig::new(
            configured(Some(1), None, None, None, None),
            // the horizon is not part of the rendered clause: it is unknown
            // until the first update cycle
            false,
        )
        .render_for_log();
        assert!(
            rendered.contains("\"src_chain_id\" = 1") && rendered.contains("\"dst_chain_id\" = 1"),
            "unexpected rendering: {rendered}"
        );
    }
}
