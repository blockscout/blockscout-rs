mod chain_id;
mod node_type;
mod rpc_url;
mod server_size;

pub use rpc_url::RpcUrl;
pub use server_size::ServerSize;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum ParsedVariableKey {
    BackendEnv(String),
    FrontendEnv(String),
    ConfigPath(String),
}

pub type ParsedVariable = (ParsedVariableKey, serde_yaml::Value);

#[async_trait::async_trait]
pub trait UserVariable<V>: Send + Sync
where
    V: Send + Sync,
{
    async fn parse_from_value(v: V) -> Result<Vec<ParsedVariable>, anyhow::Error>;

    fn validate(v: V) -> Result<(), anyhow::Error> {
        Ok(())
    }

    fn maybe_default() -> Option<V> {
        None
    }
}

pub mod macros {
    #[macro_export]
    macro_rules! var_key {
        (backend) => {
            $crate::logic::config::ParsedVariableKey::BackendEnv
        };
        (frontend) => {
            $crate::logic::config::ParsedVariableKey::FrontendEnv
        };
        (config) => {
            $crate::logic::config::ParsedVariableKey::ConfigPath
        };
        (_) => {
            compile_error!("invalid key type: `backend`, `frontend`, or `config` expected")
        };
    }
    pub use var_key;

    // #[macro_export]
    // macro_rules! impl_parse_from_value {
    //     ($key_type:ident, $key:expr) => {
    //
    //     };
    // }
    // pub use impl_parse_from_value;

    #[macro_export]
    macro_rules! single_string_env_var {
         ($var_name:ident, $key_type:ident, $key:expr, $maybe_default:expr) => {
             paste::item! {
                 pub struct [<$var_name:camel>] {}

                 #[async_trait::async_trait]
                 impl UserVariable<String> for [<$var_name:camel>] {
                     async fn parse_from_value(v: String) -> Result<Vec<ParsedVariable>, anyhow::Error> {
                         Ok(vec![
                             (
                                 $crate::logic::config::variables::macros::var_key!($key_type)($key.to_string()),
                                 serde_yaml::Value::String(v)
                             )
                         ])
                     }

                     fn maybe_default() -> Option<String> {
                         $maybe_default
                     }

                }

             }
         };

        ($var_name:ident, $key_type:ident, $key:expr, $maybe_default:expr, {$extra_body:item}) => {
            paste::item! {
                 pub struct [<$var_name:camel>] {}

                 #[async_trait::async_trait]
                 impl UserVariable<String> for [<$var_name:camel>] {
                     async fn parse_from_value(v: String) -> Result<Vec<ParsedVariable>, anyhow::Error> {
                         Ok(vec![
                             (
                                 $crate::logic::config::variables::macros::var_key!($key_type)($key.to_string()),
                                 serde_yaml::Value::String(v)
                             )
                         ])
                     }

                     fn maybe_default() -> Option<String> {
                         $maybe_default
                     }
                    $extra_body

                }

             }
        };
     }

    pub use single_string_env_var;
}
