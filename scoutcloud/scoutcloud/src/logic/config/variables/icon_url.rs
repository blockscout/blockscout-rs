use crate::logic::config::macros;

pub struct IconUrl {}

macros::single_env_var!(
    IconUrl,
    url::Url,
    frontend,
    "NEXT_PUBLIC_NETWORK_ICON",
    None
);
