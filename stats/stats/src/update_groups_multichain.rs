use crate::{
    construct_update_group,
    lines::multichain::{
        accounts_growth_multichain::*, new_txns_multichain::*,
        new_txns_multichain_window::NewTxnsMultichainWindow, txns_growth_multichain::*,
    },
    utils::singleton_groups,
};

use crate::counters::multichain::*;

singleton_groups!(
    TotalInteropMessages,
    TotalInteropTransfers,
    TotalMultichainAddresses,
    TotalMultichainTxns,
);

construct_update_group!(NewTxnsMultichainGroup {
    charts: [
        NewTxnsMultichain,
        NewTxnsMultichainWeekly,
        NewTxnsMultichainMonthly,
        NewTxnsMultichainYearly,
    ],
});

construct_update_group!(NewTxnsMultichainWindowGroup {
    charts: [NewTxnsMultichainWindow,],
});

construct_update_group!(TxnsGrowthMultichainGroup {
    charts: [
        TxnsGrowthMultichain,
        TxnsGrowthMultichainWeekly,
        TxnsGrowthMultichainMonthly,
        TxnsGrowthMultichainYearly,
    ],
});

construct_update_group!(AccountsGrowthMultichainGroup {
    charts: [
        AccountsGrowthMultichain,
        AccountsGrowthMultichainWeekly,
        AccountsGrowthMultichainMonthly,
        AccountsGrowthMultichainYearly,
    ],
});
