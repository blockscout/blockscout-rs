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
macro_rules! simple_env_var {
         ($var_name:ident, $var_ty:ty, $key_type:ident, $key:expr, $maybe_default:expr) => {
             paste::item!{
                 #[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
                 pub struct [<$var_name:camel>]($var_ty);
                 serde_plain::derive_display_from_serialize!([<$var_name:camel>]);

                 $crate::logic::config::macros::custom_env_var!($var_name, $var_ty, $key_type, $key, $maybe_default, {
                    fn new(v: $var_ty) -> Result<Self, $crate::logic::config::Error> {
                        Ok(Self(v))
                    }
                 });
             }
         };
     }
pub use simple_env_var;

#[macro_export]
macro_rules! custom_env_var {
    ($var_name:ident, $var_ty:ty, $key_type:ident, $key:expr, $maybe_default:expr, {$extra_body:item}) => {
        $crate::logic::config::macros::custom_env_var!(
            $var_name,
            $var_ty,
            [($key_type, $key)],
            $maybe_default,
            {$extra_body}
        );
    };
    ($var_name:ident, $var_ty:ty, [ $( ($key_type:ident, $key:expr) ),* ], $maybe_default:expr, {$extra_body:item}) => {
        paste::item! {
            #[allow(clippy::vec_init_then_push)]
            #[async_trait::async_trait]
            impl $crate::logic::UserVariable<$var_ty> for [<$var_name:camel>] {
                async fn build_config_vars(&self) -> Result<Vec<
                    $crate::logic::ParsedVariable>,
                    $crate::logic::config::Error
                > {
                    let mut config_vars = Vec::new();
                    $(
                        config_vars.push((
                            $crate::logic::config::macros::var_key!($key_type)($key.to_string()),
                            serde_json::Value::String(self.to_string())
                        ));
                    )*
                    Ok(config_vars)
                }

                fn maybe_default() -> Option<$var_ty> {
                    $maybe_default
                }
               $extra_body

           }

        }
    };
}

pub use custom_env_var;
