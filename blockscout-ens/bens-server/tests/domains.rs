use sqlx::PgPool;
use bens_logic::test_helpers::*;

#[sqlx::test(migrations = "../bens-logic/tests/migrations")]
async fn it_works(pool: PgPool) {
    let clients = mocked_blockscout_clients().await;
}
