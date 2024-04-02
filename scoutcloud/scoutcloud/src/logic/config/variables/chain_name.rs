use crate::logic::config::macros;

macros::simple_env_var!(ChainName, String, FrontendEnv, "NEXT_PUBLIC_NETWORK_NAME");
