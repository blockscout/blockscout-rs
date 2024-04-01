use scoutcloud_proto::blockscout::scoutcloud::v1::{
    DeployConfigInternal, DeployConfigPartialInternal,
};

pub fn config_to_json(config: &DeployConfigInternal) -> serde_json::Value {
    serde_json::json!({
        "rpc_url": config.rpc_url,
        "server_size": config.server_size,
        "chain_type": config.chain_type,
        "node_type": config.node_type,
        "chain_id": config.chain_id,
        "token_symbol": config.token_symbol,
        "instance_url": config.instance_url,
        "logo_url": config.logo_url,
        "chain_name": config.chain_name,
        "icon_url": config.icon_url,
        "homeplate_background": config.homeplate_background,
        "homeplate_text_color": config.homeplate_text_color,
    })
}

#[allow(dead_code)]
pub fn partial_config_to_json(config: &DeployConfigPartialInternal) -> serde_json::Value {
    serde_json::json!({
        "rpc_url": config.rpc_url,
        "server_size": config.server_size,
        "chain_type": config.chain_type,
        "node_type": config.node_type,
        "chain_id": config.chain_id,
        "token_symbol": config.token_symbol,
        "instance_url": config.instance_url,
        "logo_url": config.logo_url,
        "chain_name": config.chain_name,
        "icon_url": config.icon_url,
        "homeplate_background": config.homeplate_background,
        "homeplate_text_color": config.homeplate_text_color,
    })
}
