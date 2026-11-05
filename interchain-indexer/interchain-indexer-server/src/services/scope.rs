// SPDX-License-Identifier: LicenseRef-Blockscout

//! Typed read scope shared by the legacy Read API entry points and (from the
//! cluster work on) the cluster entry points. A cluster scope is fixed by the
//! config, except that a cluster request may narrow its `bridge_ids` to a
//! client selection (`narrow_bridges`). A scope is the four "cluster
//! filter fields" — `home_chain_id`, `counterparty_chain_ids`, `bridge_ids`,
//! `include_unindexed_chains` — after parsing. Handler cores take a scope, so
//! filter semantics live in one place regardless of where the scope came from.

use super::utils::{non_empty, parse_bridge_ids_csv, parse_chain_ids_csv};
use crate::config::ClusterConfig;
use std::collections::HashMap;
use tonic::Status;

/// The four cluster filter fields, typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadScope {
    pub home_chain_id: Option<i64>,
    /// `Some` only when non-empty.
    pub counterparty_chain_ids: Option<Vec<i64>>,
    /// `Some` only when non-empty.
    pub bridge_ids: Option<Vec<i32>>,
    pub include_unindexed_chains: bool,
}

impl ReadScope {
    /// Builds the scope from legacy query params. Absent or blank lists mean
    /// "no restriction". Lists are kept as sent (no sort, no dedup).
    ///
    /// Parses `bridge_ids` **before** `counterparty_chain_ids` — the order the
    /// list handlers have always used — so a request with both malformed
    /// reports the same first error as before.
    pub fn from_query(
        home_chain_id: Option<i64>,
        counterparty_chain_ids: Option<&str>,
        bridge_ids: Option<&str>,
        include_unindexed_chains: Option<bool>,
    ) -> Result<Self, Status> {
        let bridge_ids = non_empty(parse_bridge_ids_csv(bridge_ids)?);
        let counterparty_chain_ids = non_empty(parse_chain_ids_csv(
            "counterparty_chain_ids",
            counterparty_chain_ids,
        )?);
        Ok(Self {
            home_chain_id,
            counterparty_chain_ids,
            bridge_ids,
            include_unindexed_chains: include_unindexed_chains.unwrap_or(false),
        })
    }

    /// The chains this scope admits as chain-directory rows, sorted and
    /// deduplicated; `None` means unbounded.
    ///
    /// home `h` + counterparty `S` ⇒ `{h} ∪ S`; `S` alone ⇒ `S`; otherwise
    /// (home alone, or neither) unbounded.
    pub fn chain_set(&self) -> Option<Vec<i64>> {
        let mut set: Vec<i64> = match (self.home_chain_id, self.counterparty_chain_ids.as_deref()) {
            (Some(home), Some(peers)) => {
                std::iter::once(home).chain(peers.iter().copied()).collect()
            }
            (None, Some(peers)) => peers.to_vec(),
            (_, None) => return None,
        };
        set.sort_unstable();
        set.dedup();
        Some(set)
    }

    /// Rows a chain directory (`/interchain/chains`, `/stats/chains`) may
    /// return: `chain_set() ∩ requested`. `None` = all rows; `Some(empty)` =
    /// no rows (never "all").
    ///
    /// With an unbounded chain set the requested selection is returned
    /// unchanged — every legacy scope is unbounded here, so legacy behavior is
    /// untouched.
    pub fn select_rows(&self, requested: Option<Vec<i64>>) -> Option<Vec<i64>> {
        match (self.chain_set(), requested) {
            (None, requested) => requested,
            (Some(set), None) => Some(set),
            (Some(set), Some(requested)) => {
                let mut rows: Vec<i64> = requested
                    .into_iter()
                    .filter(|id| set.contains(id))
                    .collect();
                rows.sort_unstable();
                rows.dedup();
                Some(rows)
            }
        }
    }
}

/// Scope of a per-chain stats request (bridged tokens, message paths):
/// the focal chain plus an already-derived peer restriction and the bridge /
/// unindexed dimensions. Legacy entry points fill it straight from their
/// params; cluster entry points derive it from a `ReadScope`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainStatsScope {
    pub chain_id: i64,
    /// `Some` only when non-empty.
    pub counterparty_chain_ids: Option<Vec<i64>>,
    /// `Some` only when non-empty.
    pub bridge_ids: Option<Vec<i32>>,
    pub include_unindexed_chains: bool,
}

impl From<&ClusterConfig> for ReadScope {
    /// Config lists are validated non-empty at startup; normalized here
    /// (sorted, deduplicated) once.
    fn from(cluster: &ClusterConfig) -> Self {
        Self {
            home_chain_id: cluster.home_chain_id,
            counterparty_chain_ids: sorted_unique(cluster.counterparty_chain_ids.as_deref()),
            bridge_ids: sorted_unique(cluster.bridge_ids.as_deref()),
            include_unindexed_chains: cluster.include_unindexed_chains,
        }
    }
}

fn sorted_unique<T: Ord + Copy>(ids: Option<&[T]>) -> Option<Vec<T>> {
    let mut ids = ids?.to_vec();
    ids.sort_unstable();
    ids.dedup();
    (!ids.is_empty()).then_some(ids)
}

impl ReadScope {
    /// Universe derivation for a per-chain stats path chain, **cluster entry
    /// points only** (legacy routes pass their params through unchanged).
    /// `None` ⇒ `chain_id` is outside the cluster.
    ///
    /// | home | counterparty S | chain X      | focal | peers |
    /// | —    | —              | any          | X     | —     |
    /// | —    | S              | X ∈ S        | X     | S     |
    /// | h    | —              | X = h        | h     | —     |
    /// | h    | —              | X ≠ h        | X     | {h}   |
    /// | h    | S              | X = h        | h     | S     |
    /// | h    | S              | X ∈ S, X ≠ h | X     | {h}   |
    /// | otherwise                            | None          |
    pub fn chain_stats_scope(&self, chain_id: i64) -> Option<ChainStatsScope> {
        let counterparty_chain_ids =
            match (self.home_chain_id, self.counterparty_chain_ids.as_deref()) {
                (None, None) => None,
                (None, Some(peers)) if peers.contains(&chain_id) => Some(peers.to_vec()),
                (Some(home), None) => (chain_id != home).then(|| vec![home]),
                (Some(home), Some(peers)) if chain_id == home => Some(peers.to_vec()),
                (Some(home), Some(peers)) if peers.contains(&chain_id) => Some(vec![home]),
                _ => return None,
            };
        Some(ChainStatsScope {
            chain_id,
            counterparty_chain_ids,
            bridge_ids: self.bridge_ids.clone(),
            include_unindexed_chains: self.include_unindexed_chains,
        })
    }
}

/// Bridges a cluster-scoped request runs with: the client `selection` inside
/// the cluster's own bridges `own`. Outer `None` ⇒ the selection shares no
/// bridge with the cluster, so the request matches no rows — never "all
/// bridges".
///
/// | own        | selection | result                                  |
/// | any        | —         | `own`                                   |
/// | —          | `s`       | `s` as sent                             |
/// | `o`        | `s`       | `s ∩ o` sorted + deduplicated, else outer `None` |
fn narrow_bridge_ids(own: Option<&[i32]>, selection: Option<Vec<i32>>) -> Option<Option<Vec<i32>>> {
    match (own, selection) {
        (own, None) => Some(own.map(<[i32]>::to_vec)),
        (None, selection) => Some(selection),
        (Some(own), Some(selection)) => {
            let mut ids: Vec<i32> = selection
                .into_iter()
                .filter(|id| own.contains(id))
                .collect();
            ids.sort_unstable();
            ids.dedup();
            (!ids.is_empty()).then_some(Some(ids))
        }
    }
}

impl ReadScope {
    /// Narrows the scope's bridges to the client `selection`, **cluster entry
    /// points only**. `None` ⇒ no rows.
    pub fn narrow_bridges(&self, selection: Option<Vec<i32>>) -> Option<ReadScope> {
        narrow_bridge_ids(self.bridge_ids.as_deref(), selection).map(|bridge_ids| ReadScope {
            bridge_ids,
            ..self.clone()
        })
    }
}

impl ChainStatsScope {
    /// Narrows the scope's bridges to the client `selection`, **cluster entry
    /// points only**. `None` ⇒ no rows.
    pub fn narrow_bridges(self, selection: Option<Vec<i32>>) -> Option<ChainStatsScope> {
        narrow_bridge_ids(self.bridge_ids.as_deref(), selection)
            .map(|bridge_ids| ChainStatsScope { bridge_ids, ..self })
    }
}

/// Configured clusters by id, built once at startup.
#[derive(Debug, Default)]
pub struct ClusterRegistry {
    clusters: HashMap<String, ReadScope>,
}

impl ClusterRegistry {
    pub fn new(clusters: &[ClusterConfig]) -> Self {
        Self {
            clusters: clusters
                .iter()
                .map(|cluster| (cluster.id.clone(), ReadScope::from(cluster)))
                .collect(),
        }
    }

    /// The scope of `cluster_id`; unknown ⇒ `NotFound("cluster not found: <id>")`.
    pub fn scope(&self, cluster_id: &str) -> Result<&ReadScope, Status> {
        self.clusters
            .get(cluster_id)
            .ok_or_else(|| Status::not_found(format!("cluster not found: {cluster_id}")))
    }
}

#[cfg(test)]
mod tests {
    use super::{ChainStatsScope, ClusterRegistry, ReadScope, narrow_bridge_ids};
    use crate::config::ClusterConfig;
    use tonic::Code;

    fn counterparty_scope(counterparty_chain_ids: Vec<i64>) -> ReadScope {
        ReadScope {
            counterparty_chain_ids: Some(counterparty_chain_ids),
            ..Default::default()
        }
    }

    #[test]
    fn from_query_absent_and_blank_are_unrestricted() {
        assert_eq!(
            ReadScope::from_query(None, None, None, None).unwrap(),
            ReadScope::default()
        );

        let scope = ReadScope::from_query(Some(5), Some("  "), Some(""), Some(false)).unwrap();
        assert_eq!(scope.home_chain_id, Some(5));
        assert_eq!(scope.counterparty_chain_ids, None);
        assert_eq!(scope.bridge_ids, None);
        assert!(!scope.include_unindexed_chains);
    }

    #[test]
    fn from_query_keeps_lists_as_sent() {
        let scope =
            ReadScope::from_query(None, Some("100, 1,100"), Some("2,1"), Some(true)).unwrap();
        assert_eq!(scope.counterparty_chain_ids, Some(vec![100, 1, 100]));
        assert_eq!(scope.bridge_ids, Some(vec![2, 1]));
        assert!(scope.include_unindexed_chains);
    }

    #[test]
    fn from_query_reports_labeled_errors() {
        let err = ReadScope::from_query(None, Some("1,x"), None, None)
            .expect_err("malformed counterparty must be rejected");
        assert_eq!(err.code(), Code::InvalidArgument);
        assert!(
            err.message()
                .contains("invalid counterparty_chain_ids value `x`")
        );

        let err = ReadScope::from_query(None, None, Some("y"), None)
            .expect_err("malformed bridges must be rejected");
        assert_eq!(err.code(), Code::InvalidArgument);
        assert!(err.message().contains("invalid bridge_ids value `y`"));
    }

    #[test]
    fn from_query_reports_bridge_ids_first_when_both_are_malformed() {
        let err = ReadScope::from_query(None, Some("x"), Some("y"), None)
            .expect_err("malformed params must be rejected");
        assert_eq!(err.code(), Code::InvalidArgument);
        assert!(err.message().contains("invalid bridge_ids value"));
    }

    #[test]
    fn chain_set_table() {
        assert_eq!(ReadScope::default().chain_set(), None);

        let home_only = ReadScope {
            home_chain_id: Some(1),
            ..Default::default()
        };
        assert_eq!(home_only.chain_set(), None);

        assert_eq!(
            counterparty_scope(vec![100, 1, 100]).chain_set(),
            Some(vec![1, 100])
        );

        let home_and_peers = ReadScope {
            home_chain_id: Some(100),
            counterparty_chain_ids: Some(vec![1, 250, 1]),
            ..Default::default()
        };
        assert_eq!(home_and_peers.chain_set(), Some(vec![1, 100, 250]));
    }

    #[test]
    fn select_rows_unbounded_passes_request_through() {
        let scope = ReadScope::default();
        assert_eq!(scope.select_rows(None), None);
        assert_eq!(scope.select_rows(Some(vec![5, 5, 3])), Some(vec![5, 5, 3]));
    }

    #[test]
    fn select_rows_bounded_without_request_is_the_chain_set() {
        assert_eq!(
            counterparty_scope(vec![100, 1]).select_rows(None),
            Some(vec![1, 100])
        );
    }

    #[test]
    fn select_rows_intersects() {
        assert_eq!(
            counterparty_scope(vec![1, 100]).select_rows(Some(vec![100, 999, 100])),
            Some(vec![100])
        );
    }

    #[test]
    fn select_rows_disjoint_is_empty_not_all() {
        assert_eq!(
            counterparty_scope(vec![1, 100]).select_rows(Some(vec![999])),
            Some(vec![])
        );
    }

    fn scope_of(home_chain_id: Option<i64>, counterparty_chain_ids: Option<Vec<i64>>) -> ReadScope {
        ReadScope {
            home_chain_id,
            counterparty_chain_ids,
            bridge_ids: Some(vec![1, 3]),
            include_unindexed_chains: true,
        }
    }

    /// `(focal, peers)` of the derived scope; also checks the bridge and
    /// unindexed dimensions are copied.
    fn derived(scope: &ReadScope, chain_id: i64) -> Option<(i64, Option<Vec<i64>>)> {
        scope.chain_stats_scope(chain_id).map(|derived| {
            assert_eq!(derived.bridge_ids, scope.bridge_ids);
            assert_eq!(
                derived.include_unindexed_chains,
                scope.include_unindexed_chains
            );
            (derived.chain_id, derived.counterparty_chain_ids)
        })
    }

    #[test]
    fn chain_stats_scope_table() {
        // No home, no counterparty: any chain, no peers.
        assert_eq!(derived(&scope_of(None, None), 7), Some((7, None)));

        // S alone: X in S keeps S as peers, X outside is rejected.
        let peers_only = scope_of(None, Some(vec![1, 100]));
        assert_eq!(derived(&peers_only, 1), Some((1, Some(vec![1, 100]))));
        assert_eq!(derived(&peers_only, 999), None);

        // Home alone: the home chain has no peer restriction, any other chain
        // is paired with the home chain.
        let home_only = scope_of(Some(100), None);
        assert_eq!(derived(&home_only, 100), Some((100, None)));
        assert_eq!(derived(&home_only, 1), Some((1, Some(vec![100]))));

        // Home and peers.
        let home_and_peers = scope_of(Some(100), Some(vec![1, 250]));
        assert_eq!(
            derived(&home_and_peers, 100),
            Some((100, Some(vec![1, 250])))
        );
        assert_eq!(derived(&home_and_peers, 250), Some((250, Some(vec![100]))));
        assert_eq!(derived(&home_and_peers, 999), None);

        // Home inside S: the home row wins.
        let home_in_peers = scope_of(Some(1), Some(vec![1, 100]));
        assert_eq!(derived(&home_in_peers, 1), Some((1, Some(vec![1, 100]))));
    }

    fn cluster_config(id: &str) -> ClusterConfig {
        ClusterConfig {
            id: id.to_string(),
            home_chain_id: None,
            counterparty_chain_ids: None,
            bridge_ids: None,
            include_unindexed_chains: false,
        }
    }

    #[test]
    fn from_cluster_config_sorts_and_dedups() {
        let scope = ReadScope::from(&ClusterConfig {
            home_chain_id: Some(100),
            counterparty_chain_ids: Some(vec![250, 1, 250]),
            bridge_ids: Some(vec![3, 1, 3]),
            include_unindexed_chains: true,
            ..cluster_config("c")
        });
        assert_eq!(scope.home_chain_id, Some(100));
        assert_eq!(scope.counterparty_chain_ids, Some(vec![1, 250]));
        assert_eq!(scope.bridge_ids, Some(vec![1, 3]));
        assert!(scope.include_unindexed_chains);

        assert_eq!(ReadScope::from(&cluster_config("c")), ReadScope::default());
    }

    #[test]
    fn registry_unknown_cluster_is_not_found() {
        let registry = ClusterRegistry::new(&[ClusterConfig {
            bridge_ids: Some(vec![3, 1, 3]),
            ..cluster_config("known")
        }]);

        let err = registry
            .scope("nope")
            .expect_err("an unknown cluster must be rejected");
        assert_eq!(err.code(), Code::NotFound);
        assert_eq!(err.message(), "cluster not found: nope");

        let scope = registry.scope("known").expect("a configured cluster");
        assert_eq!(scope.bridge_ids, Some(vec![1, 3]));
    }

    #[test]
    fn test_narrow_bridge_ids_table_rows() {
        // no selection keeps the cluster's set (or none)
        assert_eq!(narrow_bridge_ids(None, None), Some(None));
        assert_eq!(
            narrow_bridge_ids(Some(&[1, 3]), None),
            Some(Some(vec![1, 3]))
        );
        // no cluster set: selection as sent (order and duplicates kept)
        assert_eq!(
            narrow_bridge_ids(None, Some(vec![3, 1, 3])),
            Some(Some(vec![3, 1, 3]))
        );
        // intersection, sorted and deduplicated
        assert_eq!(
            narrow_bridge_ids(Some(&[1, 2, 3]), Some(vec![3, 9, 1, 3])),
            Some(Some(vec![1, 3]))
        );
        // disjoint: no rows
        assert_eq!(narrow_bridge_ids(Some(&[1]), Some(vec![2])), None);
    }

    #[test]
    fn test_narrow_bridges_copies_other_fields_unchanged() {
        let scope = ReadScope {
            home_chain_id: Some(100),
            counterparty_chain_ids: Some(vec![1, 2]),
            bridge_ids: Some(vec![1, 3]),
            include_unindexed_chains: true,
        };
        let narrowed = scope.narrow_bridges(Some(vec![3, 7])).unwrap();
        assert_eq!(
            narrowed,
            ReadScope {
                bridge_ids: Some(vec![3]),
                ..scope.clone()
            }
        );
        assert_eq!(scope.narrow_bridges(Some(vec![7])), None);

        let stats = ChainStatsScope {
            chain_id: 5,
            counterparty_chain_ids: Some(vec![1]),
            bridge_ids: Some(vec![1, 3]),
            include_unindexed_chains: true,
        };
        let narrowed = stats.clone().narrow_bridges(Some(vec![3, 7])).unwrap();
        assert_eq!(
            narrowed,
            ChainStatsScope {
                bridge_ids: Some(vec![3]),
                ..stats.clone()
            }
        );
        assert_eq!(stats.narrow_bridges(Some(vec![7])), None);
    }
}
