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

    fn most_restrictive_from(requrements: impl Iterator<Item = Self> + Clone) -> Self;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexingStatus {
    pub blockscout: BlockscoutIndexingStatus,
    pub user_ops: UserOpsIndexingStatus,
}

impl IndexingStatusTrait for IndexingStatus {
    const MIN: Self = Self {
        blockscout: BlockscoutIndexingStatus::MIN,
        user_ops: UserOpsIndexingStatus::MIN,
    };
    const MAX: Self = Self {
        blockscout: BlockscoutIndexingStatus::MAX,
        user_ops: UserOpsIndexingStatus::MAX,
    };
    const LEAST_RESTRICTIVE: Self = Self {
        blockscout: BlockscoutIndexingStatus::LEAST_RESTRICTIVE,
        user_ops: UserOpsIndexingStatus::LEAST_RESTRICTIVE,
    };
    const MOST_RESTRICTIVE: Self = Self {
        blockscout: BlockscoutIndexingStatus::MOST_RESTRICTIVE,
        user_ops: UserOpsIndexingStatus::MOST_RESTRICTIVE,
    };

    fn is_requirement_satisfied(&self, requirement: &Self) -> bool {
        let blockscout_satisfied = self
            .blockscout
            .is_requirement_satisfied(&requirement.blockscout);
        let user_ops_satisfied = self
            .user_ops
            .is_requirement_satisfied(&requirement.user_ops);
        blockscout_satisfied && user_ops_satisfied
    }

    fn most_restrictive_from(requrements: impl Iterator<Item = Self> + Clone) -> Self {
        let blockscout_requirements = requrements.clone().map(|r| r.blockscout);
        let user_ops_requirements = requrements.map(|r| r.user_ops);
        let blockscout_most_restrictive =
            BlockscoutIndexingStatus::most_restrictive_from(blockscout_requirements);
        let user_ops_most_restrictive =
            UserOpsIndexingStatus::most_restrictive_from(user_ops_requirements);
        Self {
            blockscout: blockscout_most_restrictive,
            user_ops: user_ops_most_restrictive,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

    fn most_restrictive_from(requrements: impl Iterator<Item = Self> + Clone) -> Self {
        requrements.max().unwrap_or(Self::LEAST_RESTRICTIVE).clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

    fn most_restrictive_from(requrements: impl Iterator<Item = Self> + Clone) -> Self {
        requrements.max().unwrap_or(Self::LEAST_RESTRICTIVE).clone()
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
                        user_ops: UserOpsIndexingStatus::IndexingPastOperations
                    },
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::NoneIndexed,
                        user_ops: UserOpsIndexingStatus::IndexingPastOperations
                    }
                ]
                .into_iter()
            ),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::BlocksIndexed,
                user_ops: UserOpsIndexingStatus::IndexingPastOperations
            },
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::NoneIndexed,
                        user_ops: UserOpsIndexingStatus::IndexingPastOperations
                    },
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::BlocksIndexed,
                        user_ops: UserOpsIndexingStatus::PastOperationsIndexed
                    }
                ]
                .into_iter()
            ),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::BlocksIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed
            },
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::NoneIndexed,
                        user_ops: UserOpsIndexingStatus::PastOperationsIndexed
                    },
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                        user_ops: UserOpsIndexingStatus::IndexingPastOperations
                    }
                ]
                .into_iter()
            ),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed
            },
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(
                vec![
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                        user_ops: UserOpsIndexingStatus::PastOperationsIndexed
                    },
                    IndexingStatus {
                        blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                        user_ops: UserOpsIndexingStatus::PastOperationsIndexed
                    }
                ]
                .into_iter()
            ),
            IndexingStatus {
                blockscout: BlockscoutIndexingStatus::InternalTransactionsIndexed,
                user_ops: UserOpsIndexingStatus::PastOperationsIndexed
            },
        );

        assert_eq!(
            IndexingStatus::most_restrictive_from(vec![].into_iter()),
            IndexingStatus::LEAST_RESTRICTIVE
        );
    }
}
