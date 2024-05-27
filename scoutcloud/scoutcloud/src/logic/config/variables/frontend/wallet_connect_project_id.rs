use crate::logic::config::macros;

macros::simple_env_var!(
    WalletConnectProjectId,
    String,
    FrontendEnv,
    "NEXT_PUBLIC_WALLET_CONNECT_PROJECT_ID"
);
