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

    macro_rules! preload_domain_info {
        ($cluster:expr, $addresses:expr) => {
            let domain_infos = $cluster
                .get_domain_info($addresses.iter().map(|a| *a.hash))
                .await;

            $addresses
                .iter_mut()
                .for_each(|a| a.domain_info = domain_infos.get(&*a.hash).cloned());
        };
    }

    pub(crate) use maybe_cache_lookup;
    pub(crate) use preload_domain_info;
}
