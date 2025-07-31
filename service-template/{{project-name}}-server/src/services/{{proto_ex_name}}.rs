use crate::proto::{{proto_ex_name}}_server::*;
use crate::proto::*;
use tonic::{Request, Response, Status};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use convert_trait::TryConvert;
use {{project_name}}_logic::ApiError;
use {{project_name}}_logic::plus;

pub struct {{ProtoExName}}Impl {
    {% if database -%}
    pub db: Arc<DatabaseConnection>,
    {% endif -%}
}

#[async_trait::async_trait]
impl {{ProtoExName}} for {{ProtoExName}}Impl {
    async fn {{proto_ex_name}}_create(
        &self,
        request: Request<{{ProtoExName}}CreateRequest>,
    ) -> Result<Response<{{ProtoExName}}CreateResponse>, Status> {
        let (_metadata, _, request) = request.into_parts();
        let request: {{ProtoExName}}CreateRequestInternal = TryConvert::try_convert(request).map_err(ApiError::Convert)?;
        todo!()
    }

    async fn {{proto_ex_name}}_search(
        &self,
        request: Request<{{ProtoExName}}SearchRequest>,
    ) -> Result<Response<{{ProtoExName}}SearchResponse>, Status> {
        let items = (0..10).map(|i| {
            let id = plus(i, i); 
            Item {
                id: id.to_string(),
                name: format!("Item #{}", id),
            }
        }).collect();
        let response = {{ProtoExName}}SearchResponse {
            items,
        };
        Ok(Response::new(response))
    }
}
