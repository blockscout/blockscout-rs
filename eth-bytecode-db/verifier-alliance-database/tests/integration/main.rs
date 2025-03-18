mod contract_deployments;
mod internal_compiled_contracts;
mod transformations;
mod verified_contracts;

macro_rules! from_json {
    ($($json:tt)+) => {
        serde_json::from_value(serde_json::json!($($json)+)).unwrap()
    };
}
pub(crate) use from_json;
