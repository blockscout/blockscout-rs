use crate::logic::config::macros;

macros::simple_env_var!(RpcWsUrl, url::Url, BackendEnv, "ETHEREUM_JSONRPC_WS_URL");
