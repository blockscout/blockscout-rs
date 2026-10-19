// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::utils::derive_setters;

/// Types implementing this trait are used to both represent
/// current status and requirement for a status.
pub trait IndexingStatusTrait {
    // constants for status itself

    /// Indexing status at the start of blockscout & user ops
    const MIN: Self;
    /// Finished indexing everything
    const MAX: Self;

    // constants corresponding to status requirement

    /// The most relaxed requirement
    const LEAST_RESTRICTIVE: Self;
    /// The hardest to achieve requirement
    const MOST_RESTRICTIVE: Self;

    fn is_requirement_satisfied(&self, requirement: &Self) -> bool;

    fn most_restrictive_from(requirements: impl Iterator<Item = Self> + Clone) -> Self;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexingStatus {
    pub blockscout: BlockscoutIndexingStatus,
    pub user_ops: UserOpsIndexingStatus,
    pub zetachain_cctx: ZetachainCctxIndexingStatus,
    pub interchain: InterchainIndexingStatus,
}

derive_setters!(IndexingStatus, [
    blockscout: BlockscoutIndexingStatus,
    user_ops: UserOpsIndexingStatus,
    zetachain_cctx: ZetachainCctxIndexingStatus,
    interchain: InterchainIndexingStatus,
]);

impl IndexingStatusTrait for IndexingStatus {
    const MIN: Self = Self {
        blockscout: BlockscoutIndexingStatus::MIN,
        user_ops: UserOpsIndexingStatus::MIN,
        zetachain_cctx: ZetachainCctxIndexingStatus::MIN,
        interchain: InterchainIndexingStatus::MIN,
    };
    const MAX: Self = Self {
        blockscout: BlockscoutIndexingStatus::MAX,
        user_ops: UserOpsIndexingStatus::MAX,
        zetachain_cctx: ZetachainCctxIndexingStatus::MAX,
        interchain: InterchainIndexingStatus::MAX,
    };
    const LEAST_RESTRICTIVE: Self = Self {
        blockscout: BlockscoutIndexingStatus::LEAST_RESTRICTIVE,
        user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
        zetachain_cctx: ZetachainCctxIndexingStatus::LEAST_RESTRICTIVE,
        interchain: InterchainIndexingStatus::LEAST_RESTRICTIVE,
    };
    const MOST_RESTRICTIVE: Self = Self {
        blockscout: BlockscoutIndexingStatus::MOST_RESTRICTIVE,
        user_ops: UserOpsIndexingStatus::MOST_RESTRICTIVE,
        zetachain_cctx: ZetachainCctxIndexingStatus::MOST_RESTRICTIVE,
        interchain: InterchainIndexingStatus::MOST_RESTRICTIVE,
    };

    fn is_requirement_satisfied(&self, requirement: &Self) -> bool {
        let Self {
            blockscout,
            user_ops,
            zetachain_cctx,
            interchain,
        } = self;
        blockscout.is_requirement_satisfied(&requirement.blockscout)
            && user_ops.is_requirement_satisfied(&requirement.user_ops)
            && zetachain_cctx.is_requirement_satisfied(&requirement.zetachain_cctx)
            && interchain.is_requirement_satisfied(&requirement.interchain)
    }

    fn most_restrictive_from(requirements: impl Iterator<Item = Self> + Clone) -> Self {
        // One pass per axis over a cloned iterator, rather than a fourth level of
        // nested `unzip`. `requirements` is `Clone` by the trait bound, and each
        // axis' `most_restrictive_from` is a `max`, so this is equivalent to the
        // previous `unzip` form and stays readable if a fifth axis ever lands.
        Self {
            blockscout: BlockscoutIndexingStatus::most_restrictive_from(
                requirements.clone().map(|r| r.blockscout),
            ),
            user_ops: UserOpsIndexingStatus::most_restrictive_from(
                requirements.clone().map(|r| r.user_ops),
            ),
            zetachain_cctx: ZetachainCctxIndexingStatus::most_restrictive_from(
                requirements.clone().map(|r| r.zetachain_cctx),
            ),
            interchain: InterchainIndexingStatus::most_restrictive_from(
                requirements.map(|r| r.interchain),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BlockscoutIndexingStatus {
    NoneIndexed,
    BlocksIndexed,
    /// Implies that blocks are also indexed
    InternalTransactionsIndexed,
}

impl IndexingStatusTrait for BlockscoutIndexingStatus {
    const MIN: Self = Self::NoneIndexed;
    const MAX: Self = Self::InternalTransactionsIndexed;

    const LEAST_RESTRICTIVE: Self = Self::MIN;
    const MOST_RESTRICTIVE: Self = Self::MAX;

    fn is_requirement_satisfied(&self, requirement: &BlockscoutIndexingStatus) -> bool {
        self >= requirement
    }

    fn most_restrictive_from(requirements: impl Iterator<Item = Self> + Clone) -> Self {
        requirements.max().unwrap_or(Self::LEAST_RESTRICTIVE)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UserOpsIndexingStatus {
    IndexingPastOperations,
    PastOperationsIndexed,
}

impl IndexingStatusTrait for UserOpsIndexingStatus {
    const MIN: Self = Self::IndexingPastOperations;
    const MAX: Self = Self::PastOperationsIndexed;

    const LEAST_RESTRICTIVE: Self = Self::MIN;
    const MOST_RESTRICTIVE: Self = Self::MAX;

    fn is_requirement_satisfied(&self, requirement: &UserOpsIndexingStatus) -> bool {
        self >= requirement
    }

    fn most_restrictive_from(requirements: impl Iterator<Item = Self> + Clone) -> Self {
        requirements.max().unwrap_or(Self::LEAST_RESTRICTIVE)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ZetachainCctxIndexingStatus {
    CatchingUp,
    IndexedHistoricalData,
}

impl IndexingStatusTrait for ZetachainCctxIndexingStatus {
    const MIN: Self = Self::CatchingUp;
    const MAX: Self = Self::IndexedHistoricalData;

    const LEAST_RESTRICTIVE: Self = Self::MIN;
    const MOST_RESTRICTIVE: Self = Self::MAX;

    fn is_requirement_satisfied(&self, requirement: &ZetachainCctxIndexingStatus) -> bool {
        self >= requirement
    }

    fn most_restrictive_from(requirements: impl Iterator<Item = Self> + Clone) -> Self {
        requirements.max().unwrap_or(Self::LEAST_RESTRICTIVE)
    }
}

/// Whether the interchain indexer has caught up far enough on the `(bridge,
/// chain)` pairs relevant to the configured slice, per
/// `STATS__CONDITIONAL_START__INTERCHAIN_CATCHUP_MIN_PROGRESS`.
///
/// `CaughtUp` means "the configured **threshold** on *scanned* share is met", not
/// "all history is present": the indexer's `catchup_progress_percent` is the
/// scanned share and never a completeness measure. Named `CaughtUp` rather than
/// `IndexedHistoricalData` for exactly that reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InterchainIndexingStatus {
    CatchingUp,
    CaughtUp,
}

impl IndexingStatusTrait for InterchainIndexingStatus {
    const MIN: Self = Self::CatchingUp;
    const MAX: Self = Self::CaughtUp;

    const LEAST_RESTRICTIVE: Self = Self::MIN;
    const MOST_RESTRICTIVE: Self = Self::MAX;

    fn is_requirement_satisfied(&self, requirement: &InterchainIndexingStatus) -> bool {
        self >= requirement
    }

    fn most_restrictive_from(requirements: impl Iterator<Item = Self> + Clone) -> Self {
        requirements.max().unwrap_or(Self::LEAST_RESTRICTIVE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexing_status_requirements_are_combined_correctly() {
        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::BlocksIndexed,
                        user_ops: UserOpsIndexingStatus::IndexingPastOperations,
                        zetachain_cctx: ZetachainCctxIndexingStatus::CatchingUp,
                        interchain: InterchainIndexingStatus::CatchingUp,
                    },
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::NoneIndexed,
                        user_ops: UserOpsIndexingStatus::IndexingPastOperations,
                        zetachain_cctx: ZetachainCctxIndexingStatus::CatchingUp,
                        interchain: InterchainIndexingStatus::CatchingUp,
                    }
                ]
                .into_iter()
            ),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::BlocksIndexed,
                user_ops: UserOpsIndexingStatus::IndexingPastOperations,
                zetachain_cctx: ZetachainCctxIndexingStatus::CatchingUp,
                interchain: InterchainIndexingStatus::CatchingUp,
            },
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::NoneIndexed,
                        user_ops: UserOpsIndexingStatus::IndexingPastOperations,
                        zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
                        interchain: InterchainIndexingStatus::CatchingUp,
                    },
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::BlocksIndexed,
                        user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                        zetachain_cctx: ZetachainCctxIndexingStatus::CatchingUp,
                        interchain: InterchainIndexingStatus::CatchingUp,
                    }
                ]
                .into_iter()
            ),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::BlocksIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
                interchain: InterchainIndexingStatus::CatchingUp,
            },
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::NoneIndexed,
                        user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                        zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
                        interchain: InterchainIndexingStatus::CatchingUp,
                    },
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                        user_ops: UserOpsIndexingStatus::IndexingPastOperations,
                        zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
                        interchain: InterchainIndexingStatus::CatchingUp,
                    }
                ]
                .into_iter()
            ),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
                interchain: InterchainIndexingStatus::CatchingUp,
            },
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                        user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                        zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
                        interchain: InterchainIndexingStatus::CatchingUp,
                    },
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                        user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                        zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
                        interchain: InterchainIndexingStatus::CatchingUp,
                    }
                ]
                .into_iter()
            ),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
                interchain: InterchainIndexingStatus::CatchingUp,
            },
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(vec![].into_iter()),
            IndexingStatus::LEAST_RESTRICTIVE
        );
    }

    #[test]
    fn interchain_indexing_status_requirement_is_satisfied_by_order() {
        assert!(
            InterchainIndexingStatus::CaughtUp
                .is_requirement_satisfied(&InterchainIndexingStatus::CaughtUp)
        );
        assert!(
            InterchainIndexingStatus::CaughtUp
                .is_requirement_satisfied(&InterchainIndexingStatus::CatchingUp)
        );
        assert!(
            !InterchainIndexingStatus::CatchingUp
                .is_requirement_satisfied(&InterchainIndexingStatus::CaughtUp)
        );
        assert_eq!(
            InterchainIndexingStatus::MIN,
            InterchainIndexingStatus::LEAST_RESTRICTIVE
        );
        assert_eq!(
            InterchainIndexingStatus::MIN,
            InterchainIndexingStatus::CatchingUp
        );
        assert_eq!(
            InterchainIndexingStatus::MAX,
            InterchainIndexingStatus::MOST_RESTRICTIVE
        );
        assert_eq!(
            InterchainIndexingStatus::MAX,
            InterchainIndexingStatus::CaughtUp
        );
    }

    #[test]
    fn interchain_indexing_status_most_restrictive_from_takes_the_max() {
        assert_eq!(
            InterchainIndexingStatus::most_restrictive_from(
                vec![
                    InterchainIndexingStatus::CatchingUp,
                    InterchainIndexingStatus::CaughtUp
                ]
                .into_iter()
            ),
            InterchainIndexingStatus::CaughtUp
        );
        // the empty case is what a group with no interchain member relies on
        assert_eq!(
            InterchainIndexingStatus::most_restrictive_from(vec![].into_iter()),
            InterchainIndexingStatus::LEAST_RESTRICTIVE
        );
    }

    #[test]
    fn indexing_status_combines_the_interchain_axis_independently() {
        let a =
            IndexingStatus::LEAST_RESTRICTIVE.with_interchain(InterchainIndexingStatus::CaughtUp);
        let b =
            IndexingStatus::LEAST_RESTRICTIVE.with_interchain(InterchainIndexingStatus::CatchingUp);
        let combined = IndexingStatus::most_restrictive_from(vec![a, b].into_iter());
        assert_eq!(combined.interchain, InterchainIndexingStatus::CaughtUp);
        assert_eq!(
            combined.blockscout,
            BlockscoutIndexingStatus::LEAST_RESTRICTIVE
        );
        assert_eq!(combined.user_ops, UserOpsIndexingStatus::LEAST_RESTRICTIVE);
        assert_eq!(
            combined.zetachain_cctx,
            ZetachainCctxIndexingStatus::LEAST_RESTRICTIVE
        );
    }

    /// Pins that a seeded-satisfied axis really does clear an interchain chart's
    /// requirement — the whole basis of the "disabled costs nothing" claim.
    #[test]
    fn least_restrictive_interchain_status_does_not_gate() {
        assert!(IndexingStatus::MAX.is_requirement_satisfied(
            &IndexingStatus::LEAST_RESTRICTIVE.with_interchain(InterchainIndexingStatus::CaughtUp)
        ));
    }
}
