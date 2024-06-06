use stats::{
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
    // we want explicit naming for easier observation of duplicate names
    ($($chart: ident -> $name:literal),+ $(,)?) => {
        $(
            ::paste::paste!(
                construct_update_group!([< $chart Group >] {
                    name: $name,
                    charts: [$chart]
                });
            );
        )+
    };
}

singleton_groups!(
    ActiveAccounts -> "activeAccountsGroup",
    AverageBlockRewards -> "averageBlockRewardsGroup",
    AverageBlockSize -> "averageBlockSizeGroup",
    AverageGasLimit -> "averageGasLimitGroup",
    AverageGasPrice -> "averageGasPriceGroup",
    AverageTxnFee -> "averageTxnFeeGroup",
    GasUsedGrowth -> "gasUsedGrowthGroup",
    NativeCoinSupply -> "nativeCoinSupplyGroup",
    NewBlocks -> "newBlocksGroup",
    TxnsFee -> "txnsFeeGroup",
    TxnsSuccessRate -> "txnsSuccessRateGroup",
    AverageBlockTime -> "averageBlockTimeGroup",
    CompletedTxns -> "completedTxnsGroup",
    TotalAddresses -> "totalAddressesGroup",
    TotalBlocks -> "totalBlocksGroup",
    TotalContracts -> "totalContractsGroup",
    TotalTokens -> "totalTokensGroup",
);

construct_update_group!(AccountsIncrease {
    name: "accountsIncrease",
    charts: [AccountsGrowth, NewAccounts, TotalAccounts]
});

construct_update_group!(ContractsIncrease {
    name: "contractsIncrease",
    charts: [NewContracts, ContractsGrowth, LastNewContracts],
});

construct_update_group!(TransactionsIncrease {
    name: "transactionsIncrease",
    charts: [NewTxns, TxnsGrowth, TotalTxns],
});

construct_update_group!(VerifiedContracts {
    name: "verifiedContracts",
    charts: [
        NewVerifiedContracts,
        VerifiedContractsGrowth,
        LastNewVerifiedContracts,
        TotalVerifiedContracts
    ],
});

construct_update_group!(NativeCoinHolders {
    name: "nativeCoinHolders",
    charts: [
        NativeCoinHoldersGrowth,
        NewNativeCoinHolders,
        TotalNativeCoinHolders
    ],
});

construct_update_group!(NativeCoinTransfers {
    name: "nativeCoinTransfers",
    charts: [NewNativeCoinTransfers, TotalNativeCoinTransfers,],
});
