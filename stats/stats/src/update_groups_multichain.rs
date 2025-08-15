use crate::{
    construct_update_group,
    lines::multichain::{
        new_txns_multichain::*, new_txns_multichain_window::NewTxnsMultichainWindow,
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
