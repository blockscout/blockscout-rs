use interchain_indexer_entity::chains::Model as ChainModel;

use crate::proto::ChainInfo;

pub fn chain_model_to_proto(model: ChainModel) -> ChainInfo {
    ChainInfo {
        id: model.id.to_string(),
        name: model.name,
        logo: model.icon,
        explorer_url: model.explorer,
        custom_tx_route: model
            .custom_routes
            .clone()
            .and_then(|routes| routes.get("tx").and_then(|v| v.as_str()).map(String::from)),
        custom_address_route: model.custom_routes.clone().and_then(|routes| {
            routes
                .get("address")
                .and_then(|v| v.as_str())
                .map(String::from)
        }),
        custom_token_route: model.custom_routes.and_then(|routes| {
            routes
                .get("token")
                .and_then(|v| v.as_str())
                .map(String::from)
        }),
    }
}
