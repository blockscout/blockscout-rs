#[macro_export]
macro_rules! var_key {
    (backend) => {
        $crate::logic::ParsedVariableKey::BackendEnv
    };
    (frontend) => {
        $crate::logic::ParsedVariableKey::FrontendEnv
    };
    (config) => {
        $crate::logic::ParsedVariableKey::ConfigPath
    };
    (_) => {
        compile_error!("invalid key type: `backend`, `frontend`, or `config` expected")
    };
}
pub use var_key;

#[macro_export]
macro_rules! single_env_var {
         ($var_name:ident, $var_ty:ty, $key_type:ident, $key:expr, $maybe_default:expr) => {
             paste::item! {
                 #[async_trait::async_trait]
                 impl $crate::logic::UserVariable<$var_ty> for [<$var_name:camel>] {
                     async fn build_config_vars(v: $var_ty) -> Result<Vec<$crate::logic::ParsedVariable>, anyhow::Error> {
                         Ok(vec![
                             (
                                 $crate::logic::config::macros::var_key!($key_type)($key.to_string()),
                                 serde_json::Value::String(v.to_string())
                             )
                         ])
                     }

                     fn maybe_default() -> Option<$var_ty> {
                         $maybe_default
                     }

                }

             }
         };

        ($var_name:ident, $var_ty:ty, $key_type:ident, $key:expr, $maybe_default:expr, {$extra_body:item}) => {
            paste::item! {
                 #[async_trait::async_trait]
                 impl $crate::logic::UserVariable<$var_ty> for [<$var_name:camel>] {
                     async fn build_config_vars(v: $var_ty) -> Result<Vec<$crate::logic::ParsedVariable>, anyhow::Error> {
                         Ok(vec![
                             (
                                 $crate::logic::config::macros::var_key!($key_type)($key.to_string()),
                                 serde_json::Value::String(v.to_string())
                             )
                         ])
                     }

                     fn maybe_default() -> Option<$var_ty> {
                         $maybe_default
                     }
                    $extra_body

                }

             }
        };
     }

pub use single_env_var;
