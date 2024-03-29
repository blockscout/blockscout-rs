use crate::logic::config::macros;

macros::simple_env_var!(ChainName, String, frontend, "NEXT_PUBLIC_NETWORK_NAME");
