use crate::{
    construct_update_group,
    counters::{
        AverageBlockTime, CompletedTxns, LastNewContracts, LastNewVerifiedContracts, TotalAccounts,
        TotalAddresses, TotalBlocks, TotalContracts, TotalNativeCoinHolders,
        TotalNativeCoinTransfers, TotalTokens, TotalTxns, TotalVerifiedContracts,
    },
    lines::{
        AccountsGrowth, ActiveAccounts, AverageBlockRewards, AverageBlockRewardsWeekly,
        AverageBlockSize, AverageGasLimit, AverageGasPrice, AverageTxnFee, ContractsGrowth,
        GasUsedGrowth, NativeCoinHoldersGrowth, NativeCoinSupply, NewAccounts, NewBlocks,
        NewContracts, NewNativeCoinHolders, NewNativeCoinTransfers, NewTxns, NewVerifiedContracts,
        TxnsFee, TxnsGrowth, TxnsSuccessRate, VerifiedContractsGrowth,
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
        AverageBlockSizeWeekly,
        // AverageBlockSizeMonthly,
        // AverageBlockSizeYearly,
    ]
});

construct_update_group!(AverageGasLimitGroup {
    charts: [
        AverageGasLimit,
        AverageGasLimitWeekly,
        // AverageGasLimitMonthly,
        // AverageGasLimitYearly,
    ]
});

construct_update_group!(AverageGasPriceGroup {
    charts: [
        AverageGasPrice,
        AverageGasPriceWeekly,
        // AverageGasPriceMonthly,
        // AverageGasPriceYearly,
    ]
});

construct_update_group!(AverageTxnFeeGroup {
    charts: [
        AverageTxnFee,
        AverageTxnFeeWeekly,
        // AverageTxnFeeMonthly,
        // AverageTxnFeeYearly,
    ]
});

construct_update_group!(GasUsedGrowthGroup {
    charts: [
        GasUsedGrowth,
        GasUsedGrowthWeekly,
        // GasUsedGrowthMonthly,
        // GasUsedGrowthYearly,
    ]
});

construct_update_group!(NativeCoinSupplyGroup {
    charts: [
        NativeCoinSupply,
        NativeCoinSupplyWeekly,
        // NativeCoinSupplyMonthly,
        // NativeCoinSupplyYearly,
    ]
});

construct_update_group!(NewBlocksGroup {
    charts: [
        NewBlocks,
        NewBlocksWeekly,
        // NewBlocksMonthly,
        // NewBlocksYearly,
    ]
});

construct_update_group!(TxnsFeeGroup {
    charts: [
        TxnsFee,
        TxnsFeeWeekly,
        // TxnsFeeMonthly,
        // TxnsFeeYearly,
    ]
});

construct_update_group!(TxnsSuccessRateGroup {
    charts: [
        TxnsSuccessRate,
        TxnsSuccessRateWeekly,
        // TxnsSuccessRateMonthly,
        // TxnsSuccessRateYearly,
    ]
});

construct_update_group!(AverageBlockTimeGroup {
    charts: [
        AverageBlockTime,
        AverageBlockTimeWeekly,
        // AverageBlockTimeMonthly,
        // AverageBlockTimeYearly,
    ]
});

construct_update_group!(CompletedTxnsGroup {
    charts: [
        CompletedTxns,
        CompletedTxnsWeekly,
        // CompletedTxnsMonthly,
        // CompletedTxnsYearly,
    ]
});

construct_update_group!(TotalAddressesGroup {
    charts: [
        TotalAddresses,
        TotalAddressesWeekly,
        // TotalAddressesMonthly,
        // TotalAddressesYearly,
    ]
});

construct_update_group!(TotalBlocksGroup {
    charts: [
        TotalBlocks,
        TotalBlocksWeekly,
        // TotalBlocksMonthly,
        // TotalBlocksYearly,
    ]
});

construct_update_group!(TotalTokensGroup {
    charts: [
        TotalTokens,
        TotalTokensWeekly,
        // TotalTokensMonthly,
        // TotalTokensYearly,
    ]
});

construct_update_group!(NewAccountsGroup {
    charts: [NewAccounts, AccountsGrowth, TotalAccounts]
});

construct_update_group!(NewContractsGroup {
    charts: [
        NewContracts,
        ContractsGrowth,
        // currently can be in a separate group but after #845
        // it's expected to join this group. placing it here to avoid
        // updating config after the fix (todo: remove (#845))
        TotalContracts,
        LastNewContracts
    ],
});

construct_update_group!(NewTxnsGroup {
    charts: [NewTxns, TxnsGrowth, TotalTxns],
});

construct_update_group!(NewVerifiedContractsGroup {
    charts: [
        NewVerifiedContracts,
        VerifiedContractsGrowth,
        TotalVerifiedContracts,
        LastNewVerifiedContracts
    ],
});

construct_update_group!(NativeCoinHoldersGrowthGroup {
    charts: [
        NativeCoinHoldersGrowth,
        NewNativeCoinHolders,
        TotalNativeCoinHolders
    ],
});

construct_update_group!(NewNativeCoinTransfersGroup {
    charts: [NewNativeCoinTransfers, TotalNativeCoinTransfers],
});
