use crate::logic::config::macros;

pub struct ChainName {}

macros::single_env_var!(
    ChainName,
    String,
    frontend,
    "NEXT_PUBLIC_NETWORK_NAME",
    None
);
