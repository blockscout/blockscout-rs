use lazy_static::lazy_static;
use prometheus::{IntCounterVec, register_int_counter_vec};

lazy_static! {
    pub static ref IMPORT_ENTITIES_COUNT: IntCounterVec = register_int_counter_vec!(
        "multichain_aggregator_import_entities",
        "total number of entities requested to be imported per chain",
        &["chain_id", "entity_type"]
    )
    .unwrap();
    pub static ref CACHE_HIT_TOTAL: IntCounterVec = register_int_counter_vec!(
        "multichain_aggregator_cache_hit",
        "total number of cache hits",
        &["cache_type"]
    )
    .unwrap();
    pub static ref CACHE_MISS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "multichain_aggregator_cache_miss",
        "total number of cache misses",
        &["cache_type"]
    )
    .unwrap();
    pub static ref CACHE_REFRESH_AHEAD_TOTAL: IntCounterVec = register_int_counter_vec!(
        "multichain_aggregator_cache_refresh_ahead",
        "total number of cache refresh-ahead requests",
        &["cache_type"]
    )
    .unwrap();
}
