use super::macros;
use crate::logic::config::{ParsedVariable, UserVariable};

macros::single_string_env_var!(chain_id, backend, "CHAIN_ID", None);
