use crate::{
    construct_update_group,
    counters::{
        AverageBlockTime, CompletedTxns, LastNewContracts, LastNewVerifiedContracts, TotalAccounts,
        TotalAddresses, TotalBlocks, TotalContracts, TotalNativeCoinHolders,
        TotalNativeCoinTransfers, TotalTokens, TotalTxns, TotalVerifiedContracts,
    },
    lines::{
        AccountsGrowth, ActiveAccounts, AverageBlockRewards, AverageBlockSize, AverageGasLimit,
        AverageGasPrice, AverageTxnFee, ContractsGrowth, GasUsedGrowth, NativeCoinHoldersGrowth,
        NativeCoinSupply, NewAccounts, NewBlocks, NewContracts, NewNativeCoinHolders,
        NewNativeCoinTransfers, NewTxns, NewVerifiedContracts, TxnsFee, TxnsGrowth,
        TxnsSuccessRate, VerifiedContractsGrowth,
    },
};

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

// Group for chart `Name` is called `NameGroup`
singleton_groups!(
    ActiveAccounts,
    AverageBlockRewards,
    AverageBlockSize,
    AverageGasLimit,
    AverageGasPrice,
    AverageTxnFee,
    GasUsedGrowth,
    NativeCoinSupply,
    NewBlocks,
    TxnsFee,
    TxnsSuccessRate,
    AverageBlockTime,
    CompletedTxns,
    TotalAddresses,
    TotalBlocks,
    TotalTokens,
);

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
