use crate::{
    construct_update_group,
    counters::{
        AverageBlockTime, CompletedTxns, LastNewContracts, LastNewVerifiedContracts, TotalAccounts,
        TotalAddresses, TotalBlocks, TotalContracts, TotalNativeCoinHolders,
        TotalNativeCoinTransfers, TotalTokens, TotalTxns, TotalVerifiedContracts,
    },
    lines::{
        AccountsGrowth, AccountsGrowthWeekly, ActiveAccounts, AverageBlockRewards,
        AverageBlockRewardsWeekly, AverageBlockSize, AverageGasLimit, AverageGasPrice,
        AverageTxnFee, ContractsGrowth, GasUsedGrowth, NativeCoinHoldersGrowth, NativeCoinSupply,
        NewAccounts, NewAccountsWeekly, NewBlocks, NewContracts, NewNativeCoinHolders,
        NewNativeCoinTransfers, NewTxns, NewVerifiedContracts, TxnsFee, TxnsGrowth,
        TxnsSuccessRate, VerifiedContractsGrowth,
    },
};

// Active accounts is left without resolutions because the chart is non-trivial
// to calculate somewhat-optimally
construct_update_group!(ActiveAccountsGroup {
    charts: [ActiveAccounts]
});

construct_update_group!(AverageBlockRewardsGroup {
    charts: [
        AverageBlockRewards,
        AverageBlockRewardsWeekly,
        // AverageBlockRewardsMonthly,
        // AverageBlockRewardsYearly,
    ]
});

construct_update_group!(AverageBlockSizeGroup {
    charts: [
        AverageBlockSize,
        // AverageBlockSizeWeekly,
        // AverageBlockSizeMonthly,
        // AverageBlockSizeYearly,
    ]
});

construct_update_group!(AverageGasLimitGroup {
    charts: [
        AverageGasLimit,
        // AverageGasLimitWeekly,
        // AverageGasLimitMonthly,
        // AverageGasLimitYearly,
    ]
});

construct_update_group!(AverageGasPriceGroup {
    charts: [
        AverageGasPrice,
        // AverageGasPriceWeekly,
        // AverageGasPriceMonthly,
        // AverageGasPriceYearly,
    ]
});

construct_update_group!(AverageTxnFeeGroup {
    charts: [
        AverageTxnFee,
        // AverageTxnFeeWeekly,
        // AverageTxnFeeMonthly,
        // AverageTxnFeeYearly,
    ]
});

construct_update_group!(GasUsedGrowthGroup {
    charts: [
        GasUsedGrowth,
        // GasUsedGrowthWeekly,
        // GasUsedGrowthMonthly,
        // GasUsedGrowthYearly,
    ]
});

construct_update_group!(NativeCoinSupplyGroup {
    charts: [
        NativeCoinSupply,
        // NativeCoinSupplyWeekly,
        // NativeCoinSupplyMonthly,
        // NativeCoinSupplyYearly,
    ]
});

construct_update_group!(NewBlocksGroup {
    charts: [
        NewBlocks,
        // NewBlocksWeekly,
        // NewBlocksMonthly,
        // NewBlocksYearly,
    ]
});

construct_update_group!(TxnsFeeGroup {
    charts: [
        TxnsFee,
        // TxnsFeeWeekly,
        // TxnsFeeMonthly,
        // TxnsFeeYearly,
    ]
});

construct_update_group!(TxnsSuccessRateGroup {
    charts: [
        TxnsSuccessRate,
        // TxnsSuccessRateWeekly,
        // TxnsSuccessRateMonthly,
        // TxnsSuccessRateYearly,
    ]
});

construct_update_group!(AverageBlockTimeGroup {
    charts: [
        AverageBlockTime,
        // AverageBlockTimeWeekly,
        // AverageBlockTimeMonthly,
        // AverageBlockTimeYearly,
    ]
});

construct_update_group!(CompletedTxnsGroup {
    charts: [
        CompletedTxns,
        // CompletedTxnsWeekly,
        // CompletedTxnsMonthly,
        // CompletedTxnsYearly,
    ]
});

construct_update_group!(TotalAddressesGroup {
    charts: [
        TotalAddresses,
        // TotalAddressesWeekly,
        // TotalAddressesMonthly,
        // TotalAddressesYearly,
    ]
});

construct_update_group!(TotalBlocksGroup {
    charts: [
        TotalBlocks,
        // TotalBlocksWeekly,
        // TotalBlocksMonthly,
        // TotalBlocksYearly,
    ]
});

construct_update_group!(TotalTokensGroup {
    charts: [
        TotalTokens,
        // TotalTokensWeekly,
        // TotalTokensMonthly,
        // TotalTokensYearly,
    ]
});

construct_update_group!(NewAccountsGroup {
    charts: [
        NewAccounts,
        NewAccountsWeekly,
        // NewAccountsMonthly,
        // NewAccountsYearly,
        AccountsGrowth,
        AccountsGrowthWeekly,
        // AccountsGrowthMonthly,
        // AccountsGrowthYearly,
        TotalAccounts,
        // TotalAccountsWeekly,
        // TotalAccountsMonthly,
        // TotalAccountsYearly
    ]
});

construct_update_group!(NewContractsGroup {
    charts: [
        NewContracts,
        // NewContractsWeekly,
        // NewContractsMonthly,
        // NewContractsYearly,
        ContractsGrowth,
        // ContractsGrowthWeekly,
        // ContractsGrowthMonthly,
        // ContractsGrowthYearly,
        // currently can be in a separate group but after #845
        // it's expected to join this group. placing it here to avoid
        // updating config after the fix (todo: remove (#845))
        TotalContracts,
        // TotalContractsWeekly,
        // TotalContractsMonthly,
        // TotalContractsYearly,
        LastNewContracts,
        // LastNewContractsWeekly,
        // LastNewContractsMonthly,
        // LastNewContractsYearly
    ],
});

construct_update_group!(NewTxnsGroup {
    charts: [
        NewTxns,
        // NewTxnsWeekly,
        // NewTxnsMonthly,
        // NewTxnsYearly,
        TxnsGrowth,
        // TxnsGrowthWeekly,
        // TxnsGrowthMonthly,
        // TxnsGrowthYearly,
        TotalTxns,
        // TotalTxnsWeekly,
        // TotalTxnsMonthly,
        // TotalTxnsYearly
    ],
});

construct_update_group!(NewVerifiedContractsGroup {
    charts: [
        NewVerifiedContracts,
        // NewVerifiedContractsWeekly,
        // NewVerifiedContractsMonthly,
        // NewVerifiedContractsYearly,
        VerifiedContractsGrowth,
        // VerifiedContractsGrowthWeekly,
        // VerifiedContractsGrowthMonthly,
        // VerifiedContractsGrowthYearly,
        TotalVerifiedContracts,
        // TotalVerifiedContractsWeekly,
        // TotalVerifiedContractsMonthly,
        // TotalVerifiedContractsYearly,
        LastNewVerifiedContracts,
        // LastNewVerifiedContractsWeekly,
        // LastNewVerifiedContractsMonthly,
        // LastNewVerifiedContractsYearly
    ],
});

construct_update_group!(NativeCoinHoldersGrowthGroup {
    charts: [
        NativeCoinHoldersGrowth,
        // NativeCoinHoldersGrowthWeekly,
        // NativeCoinHoldersGrowthMonthly,
        // NativeCoinHoldersGrowthYearly,
        NewNativeCoinHolders,
        // NewNativeCoinHoldersWeekly,
        // NewNativeCoinHoldersMonthly,
        // NewNativeCoinHoldersYearly,
        TotalNativeCoinHolders,
        // TotalNativeCoinHoldersWeekly,
        // TotalNativeCoinHoldersMonthly,
        // TotalNativeCoinHoldersYearly
    ],
});

construct_update_group!(NewNativeCoinTransfersGroup {
    charts: [
        NewNativeCoinTransfers,
        // NewNativeCoinTransfersWeekly,
        // NewNativeCoinTransfersMonthly,
        // NewNativeCoinTransfersYearly,
        TotalNativeCoinTransfers,
        // TotalNativeCoinTransfersWeekly,
        // TotalNativeCoinTransfersMonthly,
        // TotalNativeCoinTransfersYearly
    ],
});
