pub fn get_min_block_multichain() -> i64 {
    // No reasonable value for min block in multichain db
    // Data is indexed for each chain separately, therefore we need some other mechanism
    // to detect that we need to recalculate the data
    // todo: implement when adding more complex multichain charts (i.e. per-address stats)
    i64::MAX
}
