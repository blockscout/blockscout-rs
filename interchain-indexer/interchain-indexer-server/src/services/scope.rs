// SPDX-License-Identifier: LicenseRef-Blockscout

//! Typed read scope shared by the legacy Read API entry points and (from the
//! cluster work on) the cluster entry points. A scope is the four "cluster
//! filter fields" — `home_chain_id`, `counterparty_chain_ids`, `bridge_ids`,
//! `include_unindexed_chains` — after parsing. Handler cores take a scope, so
//! filter semantics live in one place regardless of where the scope came from.

use super::utils::{non_empty, parse_bridge_ids_csv, parse_chain_ids_csv};
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

#[cfg(test)]
mod tests {
    use super::ReadScope;
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
}
