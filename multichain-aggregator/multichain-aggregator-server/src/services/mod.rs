mod cluster_explorer;
mod health;
mod multichain_aggregator;
mod utils;

pub use cluster_explorer::ClusterExplorer;
pub use health::HealthService;
pub use multichain_aggregator::MultichainAggregator;

pub const MULTICHAIN_CLUSTER_ID: &str = "";

pub mod macros {
    macro_rules! paginated_endpoint_base {
        ($self:expr, $inner:expr, $cluster_method:ident, $response_type:ident, $cluster:expr) => {{
            let cluster = $cluster;
            let chain_ids = parse_chain_ids($inner.chain_id)?;
            let page_size = $self.normalize_page_size($inner.page_size);
            let page_token = $inner.page_token.extract_page_token()?;

            let (items, next_page_token) = cluster
                .$cluster_method($inner.q, chain_ids, page_size as u64, page_token)
                .await?;

            Ok(Response::new($response_type {
                items: items.into_iter().map(|t| t.into()).collect(),
                next_page_params: page_token_to_proto(next_page_token, page_size),
            }))
        }};
    }

    macro_rules! paginated_list_by_query_endpoint {
        ($self:expr, $request:expr, $cluster_method:ident, $response_type:ident) => {{
            let inner = $request.into_inner();
            let cluster = $self.try_get_cluster(&inner.cluster_id)?;

            paginated_endpoint_base!($self, inner, $cluster_method, $response_type, cluster)
        }};
    }

    macro_rules! paginated_multichain_endpoint {
        ($self:expr, $request:expr, $cluster_method:ident, $response_type:ident) => {{
            let inner = $request.into_inner();
            let cluster = $self.get_multichain_cluster()?;

            paginated_endpoint_base!($self, inner, $cluster_method, $response_type, cluster)
        }};
    }

    pub(crate) use paginated_endpoint_base;
    pub(crate) use paginated_list_by_query_endpoint;
    pub(crate) use paginated_multichain_endpoint;
}
