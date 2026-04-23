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
}

derive_setters!(IndexingStatus, [
    blockscout: BlockscoutIndexingStatus,
    user_ops: UserOpsIndexingStatus,
    zetachain_cctx: ZetachainCctxIndexingStatus,
]);

impl IndexingStatusTrait for IndexingStatus {
    const MIN: Self = Self {
        blockscout: BlockscoutIndexingStatus::MIN,
        user_ops: UserOpsIndexingStatus::MIN,
        zetachain_cctx: ZetachainCctxIndexingStatus::MIN,
    };
    const MAX: Self = Self {
        blockscout: BlockscoutIndexingStatus::MAX,
        user_ops: UserOpsIndexingStatus::MAX,
        zetachain_cctx: ZetachainCctxIndexingStatus::MAX,
    };
    const LEAST_RESTRICTIVE: Self = Self {
        blockscout: BlockscoutIndexingStatus::LEAST_RESTRICTIVE,
        user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
        zetachain_cctx: ZetachainCctxIndexingStatus::LEAST_RESTRICTIVE,
    };
    const MOST_RESTRICTIVE: Self = Self {
        blockscout: BlockscoutIndexingStatus::MOST_RESTRICTIVE,
        user_ops: UserOpsIndexingStatus::MOST_RESTRICTIVE,
        zetachain_cctx: ZetachainCctxIndexingStatus::MOST_RESTRICTIVE,
    };

    fn is_requirement_satisfied(&self, requirement: &Self) -> bool {
        let Self {
            blockscout,
            user_ops,
            zetachain_cctx,
        } = self;
        let blockscout_satisfied = blockscout.is_requirement_satisfied(&requirement.blockscout);
        let user_ops_satisfied = user_ops.is_requirement_satisfied(&requirement.user_ops);
        let zetachain_cctx_satisfied =
            zetachain_cctx.is_requirement_satisfied(&requirement.zetachain_cctx);
        blockscout_satisfied && user_ops_satisfied && zetachain_cctx_satisfied
    }

    fn most_restrictive_from(requirements: impl Iterator<Item = Self> + Clone) -> Self {
        let (blockscout_reqs, (user_ops_reqs, zetachain_cctx_reqs)): (Vec<_>, (Vec<_>, Vec<_>)) =
            requirements
                .map(|r| (r.blockscout, (r.user_ops, r.zetachain_cctx)))
                .unzip();
        Self {
            blockscout: BlockscoutIndexingStatus::most_restrictive_from(
                blockscout_reqs.into_iter(),
            ),
            user_ops: UserOpsIndexingStatus::most_restrictive_from(user_ops_reqs.into_iter()),
            zetachain_cctx: ZetachainCctxIndexingStatus::most_restrictive_from(
                zetachain_cctx_reqs.into_iter(),
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
                        zetachain_cctx: ZetachainCctxIndexingStatus::CatchingUp
                    },
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::NoneIndexed,
                        user_ops: UserOpsIndexingStatus::IndexingPastOperations,
                        zetachain_cctx: ZetachainCctxIndexingStatus::CatchingUp
                    }
                ]
                .into_iter()
            ),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::BlocksIndexed,
                user_ops: UserOpsIndexingStatus::IndexingPastOperations,
                zetachain_cctx: ZetachainCctxIndexingStatus::CatchingUp
            },
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::NoneIndexed,
                        user_ops: UserOpsIndexingStatus::IndexingPastOperations,
                        zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
                    },
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::BlocksIndexed,
                        user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                        zetachain_cctx: ZetachainCctxIndexingStatus::CatchingUp
                    }
                ]
                .into_iter()
            ),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::BlocksIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData,
            },
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::NoneIndexed,
                        user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                        zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData
                    },
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                        user_ops: UserOpsIndexingStatus::IndexingPastOperations,
                        zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData
                    }
                ]
                .into_iter()
            ),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData
            },
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                        user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                        zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData
                    },
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                        user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                        zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData
                    }
                ]
                .into_iter()
            ),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed,
                zetachain_cctx: ZetachainCctxIndexingStatus::IndexedHistoricalData
            },
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(vec![].into_iter()),
            IndexingStatus::LEAST_RESTRICTIVE
        );
    }
}
