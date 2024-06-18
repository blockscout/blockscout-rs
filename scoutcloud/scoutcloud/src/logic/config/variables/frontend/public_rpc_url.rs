use crate::logic::config::macros;

macros::simple_env_var!(
    PublicRpcUrl,
    url::Url,
    FrontendEnv,
    "NEXT_PUBLIC_NETWORK_RPC_URL"
);
