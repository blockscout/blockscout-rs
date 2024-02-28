use sqlx::PgPool;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Deployment {
    pub subgraph_name: String,
    pub schema_name: String,
    pub net_version: i64,
}

async fn get_deployments(pool: &PgPool) -> Result<Vec<Deployment>, sqlx::Error> {
    sqlx::query_as!(
        Deployment,
        r#"
    select
        s.name as "subgraph_name!",
        ds.name as "schema_name!",
        c.net_version::BIGINT as "net_version!"
    from subgraphs.subgraph s
    left join subgraphs.subgraph_version sv on sv.subgraph = s.id
    left join public.deployment_schemas ds on sv.deployment = ds.subgraph
    left join public.chains c on ds.network = c.name
    order by ds.created_at
    "#,
    )
    .fetch_all(pool)
    .await
}

pub async fn subgraph_deployments(
    pool: &PgPool,
) -> Result<HashMap<i64, Vec<Deployment>>, sqlx::Error> {
    let all_deployments = get_deployments(pool).await?;

    let mut group_by_network: HashMap<i64, Vec<Deployment>> = HashMap::new();
    for item in all_deployments {
        group_by_network
            .entry(item.net_version)
            .or_default()
            .push(item);
    }
    Ok(group_by_network)
}
