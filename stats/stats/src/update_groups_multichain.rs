use crate::lines::multichain::new_txns_multichain::*;
use crate::{construct_update_group, utils::singleton_groups};

use crate::counters::multichain::*;

singleton_groups!(
    TotalInteropMessages,
    TotalInteropTransfers,
    TotalAddressesNumber,
    TotalTxnsNumber,
);

construct_update_group!(NewTxnsMultichainGroup {
    charts: [
        NewTxnsMultichain,
        NewTxnsMultichainWeekly,
        NewTxnsMultichainMonthly,
        NewTxnsMultichainYearly,
    ],
});