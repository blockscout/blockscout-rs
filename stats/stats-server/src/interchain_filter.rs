// SPDX-License-Identifier: LicenseRef-Blockscout

//! Resolving [`Settings`] into the interchain read filter, and the startup
//! guards around it.
//!
//! The predicate itself lives in the `interchain-indexer-filters` crate and the
//! stats-side wrapper in `stats::InterchainFilter`. What lives here is the
//! settings boundary: normalising operator input into a `ChainBridgeFilter`, and
//! failing fast on a configuration that cannot mean what its author intended.

use anyhow::bail;
use interchain_indexer_filters::ChainBridgeFilter;
use stats::InterchainFilterConfig;

use crate::{
    RuntimeSetup,
    settings::{InterchainFilterSettings, Mode, Settings},
};

/// Resolve `Settings` into the filter the update pipeline carries.
///
/// Normalisation mirrors the indexer's `non_empty` + `parse_chain_ids_csv`
/// (`interchain-indexer-server/src/services/utils.rs`), so that a stats config
/// and the equivalent API request produce the same `ChainBridgeFilter`:
/// - an empty list ⇒ `None`, preserving `ChainBridgeFilter`'s documented
///   "`Some` only when non-empty" invariant (a `Some(vec![])` would render
///   `is_in([])`, i.e. `1 = 2`, and silently exclude everything);
/// - `u64 → i64` and `u32 → i32` via `TryFrom`, never `as`;
/// - de-duplicated and sorted, so the rendered SQL and the fingerprint are
///   deterministic (the indexer sorts `configured_pairs` for the same reason).
///
/// `only_indexed_by_bridge` is deliberately left `None`: the observability
/// horizon is DB-derived and merged per update cycle by
/// [`InterchainFilterConfig::with_horizon`].
pub fn build_interchain_filter_config(
    settings: &Settings,
) -> anyhow::Result<InterchainFilterConfig> {
    let filter = &settings.interchain_filter;
    let home_chain_id = match effective_home_chain_id(settings) {
        Some(id) => Some(chain_id(id, "STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID")?),
        None => None,
    };
    let configured = ChainBridgeFilter {
        home_chain_id,
        counterparty_chain_ids: chain_id_list(
            filter.counterparty_chain_ids.as_deref(),
            "STATS__INTERCHAIN_FILTER__COUNTERPARTY_CHAIN_IDS",
        )?,
        src_chain_ids: chain_id_list(
            filter.src_chain_ids.as_deref(),
            "STATS__INTERCHAIN_FILTER__SRC_CHAIN_IDS",
        )?,
        dst_chain_ids: chain_id_list(
            filter.dst_chain_ids.as_deref(),
            "STATS__INTERCHAIN_FILTER__DST_CHAIN_IDS",
        )?,
        bridge_ids: bridge_id_list(
            filter.bridge_ids.as_deref(),
            "STATS__INTERCHAIN_FILTER__BRIDGE_IDS",
        )?,
        only_indexed_by_bridge: None,
    };
    Ok(InterchainFilterConfig::new(
        configured,
        filter.include_unindexed_chains,
    ))
}

/// `home_chain_id` wins; the deprecated `interchain_primary_id` is the fallback.
/// The two being set to conflicting values is rejected by
/// [`validate_interchain_filter`], so the order only matters for the equal case.
fn effective_home_chain_id(settings: &Settings) -> Option<u64> {
    settings
        .interchain_filter
        .home_chain_id
        .or(settings.interchain_primary_id)
}

fn chain_id(id: u64, env_var: &str) -> anyhow::Result<i64> {
    i64::try_from(id).map_err(|_| {
        anyhow::anyhow!("{env_var}: chain id {id} does not fit in a signed 64-bit integer")
    })
}

fn chain_id_list(ids: Option<&[u64]>, env_var: &str) -> anyhow::Result<Option<Vec<i64>>> {
    let Some(ids) = ids else { return Ok(None) };
    let mut ids = ids
        .iter()
        .map(|id| chain_id(*id, env_var))
        .collect::<anyhow::Result<Vec<i64>>>()?;
    Ok(non_empty_sorted(&mut ids))
}

fn bridge_id_list(ids: Option<&[u32]>, env_var: &str) -> anyhow::Result<Option<Vec<i32>>> {
    let Some(ids) = ids else { return Ok(None) };
    let mut ids = ids
        .iter()
        .map(|id| {
            i32::try_from(*id).map_err(|_| {
                anyhow::anyhow!("{env_var}: bridge id {id} does not fit in a signed 32-bit integer")
            })
        })
        .collect::<anyhow::Result<Vec<i32>>>()?;
    Ok(non_empty_sorted(&mut ids))
}

fn non_empty_sorted<T: Ord + Copy>(ids: &mut Vec<T>) -> Option<Vec<T>> {
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        None
    } else {
        Some(std::mem::take(ids))
    }
}

/// Fail fast on nonsense, warn on the merely surprising, and log the configured
/// filter once. Called from `server.rs` next to
/// `check_if_unsupported_charts_are_enabled`, before anything is spawned.
///
/// The warnings all describe configurations the indexer's own read API accepts,
/// so stats accepts them too — parity beats tidiness. Only the three hard errors
/// below describe a configuration that cannot mean what its author intended.
pub async fn validate_interchain_filter(
    settings: &Settings,
    setup: &RuntimeSetup,
) -> anyhow::Result<InterchainFilterConfig> {
    if settings.mode != Mode::Interchain {
        // hard error 1
        if let Some(env_var) = first_set_field(&settings.interchain_filter) {
            bail!(
                "{env_var} is set, but STATS__MODE is {:?}. The interchain read filter \
                 only applies in `interchain` mode; unset it or switch modes.",
                settings.mode
            );
        }
        // nothing else here is meaningful outside interchain mode, and
        // `interchain_primary_id` in another mode is pre-existing (ignored) behaviour
        return build_interchain_filter_config(settings);
    }

    let filter = &settings.interchain_filter;
    // hard error 2 (and warnings 4 and 5)
    match (settings.interchain_primary_id, filter.home_chain_id) {
        (Some(deprecated), Some(current)) if deprecated != current => bail!(
            "STATS__INTERCHAIN_PRIMARY_ID ({deprecated}) and \
             STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID ({current}) are both set to different \
             values. STATS__INTERCHAIN_PRIMARY_ID is deprecated; set only \
             STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID."
        ),
        (Some(_), Some(_)) => tracing::warn!(
            "STATS__INTERCHAIN_PRIMARY_ID is deprecated and set to the same value as \
             STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID; ignoring the deprecated var"
        ),
        (Some(deprecated), None) => tracing::warn!(
            "STATS__INTERCHAIN_PRIMARY_ID is deprecated; it is honoured as \
             STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID={deprecated}. Please migrate."
        ),
        (None, _) => {}
    }

    // hard error 3 lives inside the conversion
    let config = build_interchain_filter_config(settings)?;
    let home_chain_id = effective_home_chain_id(settings);

    // warning 6
    if let (Some(home), Some(counterparties)) =
        (home_chain_id, filter.counterparty_chain_ids.as_deref())
        && counterparties.contains(&home)
    {
        tracing::warn!(
            "STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID ({home}) also appears in \
             STATS__INTERCHAIN_FILTER__COUNTERPARTY_CHAIN_IDS, which admits the route \
             {home} -> {home}. No such message can exist."
        );
    }

    // warning 7
    if home_chain_id.is_none() && filter.counterparty_chain_ids.is_some() {
        tracing::warn!(
            "STATS__INTERCHAIN_FILTER__COUNTERPARTY_CHAIN_IDS is set without \
             STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID. That is NOT a focal filter: it means \
             `src IN set AND dst IN set`, a conjunction keeping only routes with both \
             endpoints inside the set."
        );
    }

    // warning 8
    if home_chain_id.is_none() {
        let directional: Vec<String> = setup
            .update_groups
            .values()
            .flat_map(|g| g.group.enabled_members_with_deps(&g.enabled_members))
            .map(|key| key.name().to_owned())
            .filter(|name| name.contains("Sent") || name.contains("Received"))
            .collect();
        if !directional.is_empty() {
            let mut names: Vec<String> = directional;
            names.sort_unstable();
            names.dedup();
            tracing::warn!(
                "STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID is not set, but directional charts are \
                 enabled: {}. Without a focal chain they mean \"source-side observed\" / \
                 \"destination-side observed\", not a direction relative to any chain.",
                names.join(", ")
            );
        }
    }

    // warning 9
    if let (Some(home), Some(src), Some(dst)) = (
        home_chain_id,
        filter.src_chain_ids.as_deref(),
        filter.dst_chain_ids.as_deref(),
    ) && !src.contains(&home)
        && !dst.contains(&home)
    {
        tracing::warn!(
            "STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID ({home}) appears in neither \
             STATS__INTERCHAIN_FILTER__SRC_CHAIN_IDS nor \
             STATS__INTERCHAIN_FILTER__DST_CHAIN_IDS. The focal term requires {home} on one \
             endpoint and those terms forbid it on both, so this filter matches nothing."
        );
    }

    // "configured", not "effective": with `include_unindexed_chains = false` the
    // update cycle ANDs an observability horizon that this line cannot show,
    // because it is read from the indexer DB and is not known yet — so the
    // rendered clause can read `<unfiltered>` while the applied predicate is
    // restrictive. `update_service` logs the resolved horizon at DEBUG.
    tracing::info!(
        include_unindexed_chains = config.include_unindexed_chains(),
        "configured interchain read filter: {}",
        config.render_for_log()
    );

    Ok(config)
}

/// The env var name of the first non-default field, for the mode guard.
fn first_set_field(filter: &InterchainFilterSettings) -> Option<&'static str> {
    let InterchainFilterSettings {
        home_chain_id,
        counterparty_chain_ids,
        src_chain_ids,
        dst_chain_ids,
        bridge_ids,
        include_unindexed_chains,
    } = filter;
    // destructured rather than field-accessed so that a seventh field cannot be
    // added without this guard failing to compile
    [
        home_chain_id
            .is_some()
            .then_some("STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID"),
        counterparty_chain_ids
            .is_some()
            .then_some("STATS__INTERCHAIN_FILTER__COUNTERPARTY_CHAIN_IDS"),
        src_chain_ids
            .is_some()
            .then_some("STATS__INTERCHAIN_FILTER__SRC_CHAIN_IDS"),
        dst_chain_ids
            .is_some()
            .then_some("STATS__INTERCHAIN_FILTER__DST_CHAIN_IDS"),
        bridge_ids
            .is_some()
            .then_some("STATS__INTERCHAIN_FILTER__BRIDGE_IDS"),
        (*include_unindexed_chains != InterchainFilterSettings::default().include_unindexed_chains)
            .then_some("STATS__INTERCHAIN_FILTER__INCLUDE_UNINDEXED_CHAINS"),
    ]
    .into_iter()
    .flatten()
    .next()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        config::{self, types::AllChartSettings},
        settings::InterchainFilterSettings,
    };

    fn interchain_settings(filter: InterchainFilterSettings) -> Settings {
        Settings {
            mode: Mode::Interchain,
            interchain_filter: filter,
            ..Settings::default()
        }
    }

    /// A runtime setup with every chart disabled — enough for the validator,
    /// which only reads the *enabled* chart names (warning 8).
    fn empty_setup() -> RuntimeSetup {
        RuntimeSetup::new(
            config::charts::Config::<AllChartSettings> {
                counters: Default::default(),
                lines: Default::default(),
            },
            config::layout::Config {
                counters_order: vec![],
                line_chart_categories: vec![],
            },
            config::update_groups::Config {
                schedules: Default::default(),
            },
        )
        .unwrap()
    }

    #[tokio::test]
    async fn rejects_filter_settings_outside_interchain_mode() {
        let settings = Settings {
            mode: Mode::Blockscout,
            interchain_filter: InterchainFilterSettings {
                home_chain_id: Some(1),
                ..Default::default()
            },
            ..Settings::default()
        };
        let err = validate_interchain_filter(&settings, &empty_setup())
            .await
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID"),
            "the error must name the offending var: {err}"
        );
    }

    #[tokio::test]
    async fn rejects_include_unindexed_chains_outside_interchain_mode() {
        let settings = Settings {
            mode: Mode::MultichainAggregator,
            interchain_filter: InterchainFilterSettings {
                include_unindexed_chains: true,
                ..Default::default()
            },
            ..Settings::default()
        };
        let err = validate_interchain_filter(&settings, &empty_setup())
            .await
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("STATS__INTERCHAIN_FILTER__INCLUDE_UNINDEXED_CHAINS"),
            "the error must name the offending var: {err}"
        );
    }

    #[tokio::test]
    async fn default_filter_settings_are_allowed_in_every_mode() {
        for mode in [
            Mode::Blockscout,
            Mode::MultichainAggregator,
            Mode::Zetachain,
            Mode::Interchain,
        ] {
            let settings = Settings {
                mode,
                ..Settings::default()
            };
            validate_interchain_filter(&settings, &empty_setup())
                .await
                .unwrap_or_else(|err| panic!("{mode:?} rejected the default filter: {err}"));
        }
    }

    /// The five-row back-compat table for the deprecated var.
    #[tokio::test]
    async fn interchain_primary_id_back_compat_table() {
        let cases: [(Option<u64>, Option<u64>, Option<i64>); 4] = [
            (None, None, None),
            (Some(1), None, Some(1)),
            (None, Some(2), Some(2)),
            (Some(3), Some(3), Some(3)),
        ];
        for (deprecated, current, expected) in cases {
            let settings = Settings {
                interchain_primary_id: deprecated,
                ..interchain_settings(InterchainFilterSettings {
                    home_chain_id: current,
                    ..Default::default()
                })
            };
            let config = validate_interchain_filter(&settings, &empty_setup())
                .await
                .unwrap_or_else(|err| panic!("{deprecated:?}/{current:?} rejected: {err}"));
            assert_eq!(
                config.with_horizon(None).home_chain_id(),
                expected,
                "{deprecated:?}/{current:?}"
            );
        }
        // the fifth row: both set, to different values
        let settings = Settings {
            interchain_primary_id: Some(1),
            ..interchain_settings(InterchainFilterSettings {
                home_chain_id: Some(2),
                ..Default::default()
            })
        };
        let err = validate_interchain_filter(&settings, &empty_setup())
            .await
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("STATS__INTERCHAIN_PRIMARY_ID")
                && err.contains("STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID"),
            "the error must name both vars: {err}"
        );
    }

    #[test]
    fn rejects_ids_that_do_not_fit_the_signed_column_types() {
        let too_big_chain = interchain_settings(InterchainFilterSettings {
            src_chain_ids: Some(vec![u64::MAX]),
            ..Default::default()
        });
        let err = build_interchain_filter_config(&too_big_chain)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("STATS__INTERCHAIN_FILTER__SRC_CHAIN_IDS"),
            "{err}"
        );

        let too_big_home = interchain_settings(InterchainFilterSettings {
            home_chain_id: Some(u64::MAX),
            ..Default::default()
        });
        let err = build_interchain_filter_config(&too_big_home)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("STATS__INTERCHAIN_FILTER__HOME_CHAIN_ID"),
            "{err}"
        );

        let too_big_bridge = interchain_settings(InterchainFilterSettings {
            bridge_ids: Some(vec![u32::MAX]),
            ..Default::default()
        });
        let err = build_interchain_filter_config(&too_big_bridge)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("STATS__INTERCHAIN_FILTER__BRIDGE_IDS"),
            "{err}"
        );
    }

    #[test]
    fn normalises_lists_to_sorted_deduplicated_and_non_empty() {
        let settings = interchain_settings(InterchainFilterSettings {
            counterparty_chain_ids: Some(vec![137, 10, 137]),
            // an empty list must become `None`, never `Some([])` — the latter
            // renders `is_in([])`, i.e. `1 = 2`
            src_chain_ids: Some(vec![]),
            bridge_ids: Some(vec![7, 3, 7]),
            ..Default::default()
        });
        let filter = build_interchain_filter_config(&settings)
            .unwrap()
            .with_horizon(None);
        assert_eq!(
            filter.condition_source.counterparty_chain_ids,
            Some(vec![10, 137])
        );
        assert_eq!(filter.condition_source.src_chain_ids, None);
        assert_eq!(filter.condition_source.bridge_ids, Some(vec![3, 7]));
    }

    #[test]
    fn normalisation_makes_the_fingerprint_order_independent() {
        let one = build_interchain_filter_config(&interchain_settings(InterchainFilterSettings {
            counterparty_chain_ids: Some(vec![10, 137]),
            ..Default::default()
        }))
        .unwrap();
        let other =
            build_interchain_filter_config(&interchain_settings(InterchainFilterSettings {
                counterparty_chain_ids: Some(vec![137, 10, 10]),
                ..Default::default()
            }))
            .unwrap();
        assert_eq!(one, other);
    }

    #[test]
    fn include_unindexed_chains_reaches_the_config_and_the_fingerprint() {
        let restricted = build_interchain_filter_config(&interchain_settings(
            InterchainFilterSettings::default(),
        ))
        .unwrap();
        let permissive =
            build_interchain_filter_config(&interchain_settings(InterchainFilterSettings {
                include_unindexed_chains: true,
                ..Default::default()
            }))
            .unwrap();
        assert!(!restricted.include_unindexed_chains());
        assert!(permissive.include_unindexed_chains());
        assert_ne!(
            restricted.with_horizon(None).fingerprint,
            permissive.with_horizon(None).fingerprint,
            "flipping include_unindexed_chains must change the fingerprint"
        );
    }
}
