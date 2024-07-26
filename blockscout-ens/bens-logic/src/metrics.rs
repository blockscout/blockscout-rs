use lazy_static::lazy_static;
use prometheus::{register_int_counter, IntCounter};

lazy_static! {
    pub static ref WILDCARD_RESOLVE_ATTEMPTS: IntCounter = register_int_counter!(
        "bens_wildcard_resolve_attemps",
        "total attempts to resolve domain with wildcard resolver",
    )
    .unwrap();
    pub static ref WILDCARD_RESOLVE_SUCCESS: IntCounter = register_int_counter!(
        "bens_wildcard_resolve_success",
        "total successful attempts to resolve domain with wildcard resolver",
    )
    .unwrap();
}
