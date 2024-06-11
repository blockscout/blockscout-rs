use super::mock::mock_blockscout;
use httpmock::MockServer;
use sea_orm::{prelude::*, ConnectionTrait};
use serde_json::json;

pub async fn start_blockscout_and_set_url<C: ConnectionTrait>(
    db: &C,
    healthy: bool,
    indexed: bool,
) -> MockServer {
    let blockscout = mock_blockscout(healthy, indexed);
    let blockscout_url = blockscout.base_url();
    update_blockscout_url_of_all_instances(db, &blockscout_url).await;
    blockscout
}

pub async fn update_blockscout_url_of_all_instances<C: ConnectionTrait>(
    db: &C,
    blockscout_url: &str,
) {
    let parsed_config_raw = json!({"frontend": {"ingress": {"hostname": blockscout_url}}});
    scoutcloud_entity::deployments::Entity::update_many()
        .col_expr(
            scoutcloud_entity::deployments::Column::ParsedConfig,
            Expr::value(parsed_config_raw),
        )
        .exec(db)
        .await
        .expect("failed to update deployment");
}
