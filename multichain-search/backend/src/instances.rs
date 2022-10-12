use actix_web::{
    web::{Data, Json},
    HttpRequest,
};
use serde::Serialize;

use crate::proxy::{self, Instance};

#[derive(Serialize)]
pub struct InstancesResponse {
    pub items: Vec<Instance>,
}

pub async fn get_instances(
    _request: HttpRequest,
    proxy: Data<proxy::BlockscoutProxy>,
) -> Json<InstancesResponse> {
    let items = proxy.instances();
    Json(InstancesResponse { items })
}
