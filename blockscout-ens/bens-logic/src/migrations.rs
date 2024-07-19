#[tracing::instrument(skip_all, level = "INFO")]
pub async fn run(db: &sqlx::PgPool) -> Result<(), anyhow::Error> {
    sqlx::migrate!().run(db).await?;
    Ok(())
}
