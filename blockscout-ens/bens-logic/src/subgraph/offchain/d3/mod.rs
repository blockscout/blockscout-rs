use crate::{entity::subgraph::domain::CreationDomain, protocols::DomainNameOnProtocol};
use sqlx::PgPool;

pub async fn maybe_offchain_resolution(
    _db: &PgPool,
    _from_user: &DomainNameOnProtocol<'_>,
) -> Option<CreationDomain> {
    None
}
