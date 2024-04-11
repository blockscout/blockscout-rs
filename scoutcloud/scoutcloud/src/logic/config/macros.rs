#[macro_export]
macro_rules! simple_env_var {
        ($var_name:ident, $var_ty:ty, $key_type:ident, $key:expr) => {
            $crate::logic::config::macros::simple_env_var!(
                $var_name,
                $var_ty,
                $key_type,
                $key,
                {}
            );
        };
        ($var_name:ident, $var_ty:ty, $key_type:ident, $key:expr, {$($extra_body:tt)*}) => {
             paste::item!{
                 #[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
                 pub struct [<$var_name:camel>]($var_ty);
                 serde_plain::derive_display_from_serialize!([<$var_name:camel>]);

                 $crate::logic::config::macros::custom_env_var!($var_name, $var_ty, $key_type, $key, {
                    fn new(
                        v: $var_ty,
                        _context: &$crate::logic::ConfigValidationContext
                    ) -> Result<Self, $crate::logic::config::Error> {
                        Ok(Self(v))
                    }
                    $($extra_body)*
                 });
             }
         };
     }
pub use simple_env_var;

#[macro_export]
macro_rules! custom_env_var {
    ($var_name:ident, $var_ty:ty, $key_type:ident, $key:expr, {$($extra_body:tt)*}) => {
        $crate::logic::config::macros::custom_env_var!(
            $var_name,
            $var_ty,
            [($key_type, $key)],
            {$($extra_body)*}
        );
    };
    ($var_name:ident, $var_ty:ty, [ $( ($key_type:ident, $key:expr) ),* ], {$($extra_body:tt)*}) => {
        paste::item! {
            #[allow(clippy::vec_init_then_push)]
            #[async_trait::async_trait]
            impl $crate::logic::UserVariable for [<$var_name:camel>] {
                type SourceType = $var_ty;

                async fn build_config_vars(&self, _context: &$crate::logic::ConfigValidationContext) -> Result<Vec<
                    $crate::logic::ParsedVariable>,
                    $crate::logic::config::Error
                > {
                    let mut config_vars = Vec::new();
                    $(
                        config_vars.push((
                            $crate::logic::ParsedVariableKey::$key_type($key.to_string()),
                            serde_json::Value::String(self.to_string())
                        ));
                    )*
                    Ok(config_vars)
                }

               $($extra_body)*

           }

        }
    };
}

pub use custom_env_var;
