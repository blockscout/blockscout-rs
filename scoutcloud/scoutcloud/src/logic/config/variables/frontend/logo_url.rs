use crate::logic::config::macros;

macros::simple_env_var!(LogoUrl, url::Url, FrontendEnv, "NEXT_PUBLIC_NETWORK_LOGO");
