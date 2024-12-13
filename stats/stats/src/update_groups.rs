use crate::{construct_update_group, counters::*, lines::*};

macro_rules! singleton_groups {
    ($($chart: ident),+ $(,)?) => {
        $(
            ::paste::paste!(
                construct_update_group!([< $chart Group >] {
                    charts: [$chart]
                });
            );
        )+
    };
}

// Mostly counters because they don't have resolutions
// Group for chart `Name` is called `NameGroup`
singleton_groups!(
    // Active accounts is left without resolutions because the chart is non-trivial
    // to calculate somewhat-optimally
    ActiveAccounts,
    AverageBlockTime,
    CompletedTxns,
    TotalAddresses,
    TotalBlocks,
    TotalTxns,
    // Even though it depends on `TotalTxns` and `TotalBlocks`,
    // it's ok not to update it as frequently.
    // Granular control over these 2 still seems useful.
    TotalOperationalTxns,
    TotalTokens,
    YesterdayTxns,
    // Each of the `ActiveRecurringAccounts*` charts includes quite heavy SQL query,
    // thus it's better to have granular control on update times.
    ActiveRecurringAccountsDailyRecurrence60Days,
    ActiveRecurringAccountsMonthlyRecurrence60Days,
    ActiveRecurringAccountsWeeklyRecurrence60Days,
    ActiveRecurringAccountsYearlyRecurrence60Days,
    ActiveRecurringAccountsDailyRecurrence90Days,
    ActiveRecurringAccountsMonthlyRecurrence90Days,
    ActiveRecurringAccountsWeeklyRecurrence90Days,
    ActiveRecurringAccountsYearlyRecurrence90Days,
    ActiveRecurringAccountsDailyRecurrence120Days,
    ActiveRecurringAccountsMonthlyRecurrence120Days,
    ActiveRecurringAccountsWeeklyRecurrence120Days,
    ActiveRecurringAccountsYearlyRecurrence120Days,
);

construct_update_group!(AverageBlockRewardsGroup {
    charts: [
        AverageBlockRewards,
        AverageBlockRewardsWeekly,
        AverageBlockRewardsMonthly,
        AverageBlockRewardsYearly,
    ]
});

construct_update_group!(AverageBlockSizeGroup {
    charts: [
        AverageBlockSize,
        AverageBlockSizeWeekly,
        AverageBlockSizeMonthly,
        AverageBlockSizeYearly,
    ]
});

construct_update_group!(AverageGasLimitGroup {
    charts: [
        AverageGasLimit,
        AverageGasLimitWeekly,
        AverageGasLimitMonthly,
        AverageGasLimitYearly,
    ]
});

construct_update_group!(AverageGasPriceGroup {
    charts: [
        AverageGasPrice,
        AverageGasPriceWeekly,
        AverageGasPriceMonthly,
        AverageGasPriceYearly,
    ]
});

construct_update_group!(AverageTxnFeeGroup {
    charts: [
        AverageTxnFee,
        AverageTxnFeeWeekly,
        AverageTxnFeeMonthly,
        AverageTxnFeeYearly,
    ]
});

construct_update_group!(GasUsedGrowthGroup {
    charts: [
        GasUsedGrowth,
        GasUsedGrowthWeekly,
        GasUsedGrowthMonthly,
        GasUsedGrowthYearly,
    ]
});

construct_update_group!(NativeCoinSupplyGroup {
    charts: [
        NativeCoinSupply,
        NativeCoinSupplyWeekly,
        NativeCoinSupplyMonthly,
        NativeCoinSupplyYearly,
    ]
});

construct_update_group!(NewBlocksGroup {
    charts: [
        NewBlocks,
        NewBlocksWeekly,
        NewBlocksMonthly,
        NewBlocksYearly,
    ]
});

construct_update_group!(TxnsFeeGroup {
    charts: [TxnsFee, TxnsFeeWeekly, TxnsFeeMonthly, TxnsFeeYearly,]
});

construct_update_group!(TxnsSuccessRateGroup {
    charts: [
        TxnsSuccessRate,
        TxnsSuccessRateWeekly,
        TxnsSuccessRateMonthly,
        TxnsSuccessRateYearly,
    ]
});

construct_update_group!(NewAccountsGroup {
    charts: [
        NewAccounts,
        NewAccountsWeekly,
        NewAccountsMonthly,
        NewAccountsYearly,
        AccountsGrowth,
        AccountsGrowthWeekly,
        AccountsGrowthMonthly,
        AccountsGrowthYearly,
        TotalAccounts,
    ]
});

construct_update_group!(NewContractsGroup {
    charts: [
        NewContracts,
        NewContractsWeekly,
        NewContractsMonthly,
        NewContractsYearly,
        ContractsGrowth,
        ContractsGrowthWeekly,
        ContractsGrowthMonthly,
        ContractsGrowthYearly,
        // currently can be in a separate group but after #845
        // it's expected to join this group. placing it here to avoid
        // updating config after the fix (todo: remove (#845))
        TotalContracts,
        LastNewContracts,
    ],
});

construct_update_group!(NewTxnsGroup {
    charts: [
        NewTxns,
        NewTxnsWeekly,
        NewTxnsMonthly,
        NewTxnsYearly,
        TxnsGrowth,
        TxnsGrowthWeekly,
        TxnsGrowthMonthly,
        TxnsGrowthYearly,
    ],
});

construct_update_group!(NewVerifiedContractsGroup {
    charts: [
        NewVerifiedContracts,
        NewVerifiedContractsWeekly,
        NewVerifiedContractsMonthly,
        NewVerifiedContractsYearly,
        VerifiedContractsGrowth,
        VerifiedContractsGrowthWeekly,
        VerifiedContractsGrowthMonthly,
        VerifiedContractsGrowthYearly,
        TotalVerifiedContracts,
        LastNewVerifiedContracts,
    ],
});

construct_update_group!(NativeCoinHoldersGrowthGroup {
    charts: [
        NativeCoinHoldersGrowth,
        NativeCoinHoldersGrowthWeekly,
        NativeCoinHoldersGrowthMonthly,
        NativeCoinHoldersGrowthYearly,
        NewNativeCoinHolders,
        NewNativeCoinHoldersWeekly,
        NewNativeCoinHoldersMonthly,
        NewNativeCoinHoldersYearly,
        TotalNativeCoinHolders,
    ],
});

construct_update_group!(NewNativeCoinTransfersGroup {
    charts: [
        NewNativeCoinTransfers,
        NewNativeCoinTransfersWeekly,
        NewNativeCoinTransfersMonthly,
        NewNativeCoinTransfersYearly,
        TotalNativeCoinTransfers,
    ],
});
