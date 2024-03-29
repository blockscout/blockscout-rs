use crate::logic::config::macros;

macros::simple_env_var!(IconUrl, url::Url, frontend, "NEXT_PUBLIC_NETWORK_ICON");
