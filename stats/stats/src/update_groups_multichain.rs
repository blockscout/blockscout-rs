use crate::utils::singleton_groups;

use crate::counters::multichain::*;

singleton_groups!(
    TotalInteropMessages,
    TotalInteropTransfers,
    TotalAddressesNumber,
    TotalTxnsNumber,
);
