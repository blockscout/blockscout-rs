// amount of active until day `date`
pub const ACCOUNTS_GROWTH: &str = "accountsGrowth";

// amount of active accounts at day `date`
pub const ACTIVE_ACCOUNTS: &str = "activeAccounts";

// arithmetic mean of block sizes (in bytes) at day `date`
pub const AVERAGE_BLOCK_SIZE: &str = "averageBlockSize";

// arithmetic mean of gas limit in blocks at day `date`
pub const AVERAGE_GAS_LIMIT: &str = "averageGasLimit";

// arithmetic mean of gas price in transactions at day `date`
pub const AVERAGE_GAS_PRICE: &str = "averageGasPrice";

// arithmetic mean of fee (IN USD) in transactions at day `date`
// TODO: how to get value of token in USD?
pub const AVERAGE_TXN_FEE: &str = "averageTxnFee";

// amount of used gas of all blocks until day `date`
pub const GAS_USED_GROWTH: &str = "gasUsedGrowth";

// amount of accounts that have native coins until day `date`
pub const NATIVE_COIN_HOLDERS_GROWTH: &str = "nativeCoinHoldersGrowth";

// sum of all account balances until day `date`
pub const NATIVE_COIN_SUPPLY: &str = "nativeCoinSupply";

// amount of new block at day `date`
pub const NEW_BLOCKS: &str = "newBlocks";

// amount of new transactions with transfering native tokens at day `date`
pub const NEW_NATIVE_COINS_TRANSFERS: &str = "newNativeCoinTransfers";

// amount of new transactions (contract calls, native coin transfers) at day `date`
pub const NEW_TXNS: &str = "newTxns";

// amount of ether paid as transaction fee at day `date`
pub const TXNS_FEE: &str = "txnsFee";

// amount of transactions until day `date`
pub const TXNS_GROWTH: &str = "txnsGrowth";
