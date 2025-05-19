use lazy_static::lazy_static;
use prometheus::{register_int_counter_vec, IntCounterVec};

lazy_static! {
    pub static ref IMPORT_ENTITIES_COUNT: IntCounterVec = register_int_counter_vec!(
        "multichain_aggregator_import_entities",
        "total number of entities requested to be imported per chain",
        &["chain_id", "entity_type"]
    )
    .unwrap();
}
