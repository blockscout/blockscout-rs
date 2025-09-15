pub mod api_key_manager;
pub mod chains;
pub mod channel;
pub mod cluster;
pub mod dapp_search;
pub mod import;
pub mod quick_search;

pub const MIN_QUERY_LENGTH: usize = 3;

pub mod macros {
    macro_rules! maybe_cache_lookup {
        ($cache:expr, $key:expr, $get:expr) => {
            if let Some(cache) = $cache {
                cache
                    .default_request()
                    .key($key)
                    .execute($get)
                    .await
                    .map_err(|err| err.into())
            } else {
                $get().await
            }
        };
    }

    pub(crate) use maybe_cache_lookup;
}
