use crate::logic::config::macros;

macros::simple_env_var!(IconUrl, url::Url, FrontendEnv, "NEXT_PUBLIC_NETWORK_ICON");
