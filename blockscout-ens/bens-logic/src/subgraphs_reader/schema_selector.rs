use std::collections::HashMap;

use sqlx::PgPool;

pub async fn schema_names(pool: &PgPool) -> Result<HashMap<i64, String>, sqlx::Error> {
    let res = sqlx::query!(
        "
SELECT DISTINCT ON (c.net_version) 
    c.net_version::BIGINT as net_version,
    ds.name AS schema_name
FROM deployment_schemas ds
LEFT JOIN chains c ON ds.network = c.NAME
ORDER  BY c.net_version,
ds.version DESC;",
    )
    .fetch_all(pool)
    .await?;

    Ok(res
        .into_iter()
        .filter_map(|r| {
            r.net_version
                .map(|net_version| (net_version, r.schema_name))
        })
        .collect())
}
