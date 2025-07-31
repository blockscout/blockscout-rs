use crate::proto::{{proto_ex_name}}_server::*;
use crate::proto::*;
use tonic::{Request, Response, Status};

#[derive(Default)]
pub struct {{ProtoExName}}Impl {}

#[async_trait::async_trait]
impl {{ProtoExName}} for {{ProtoExName}}Impl {
    async fn {{proto_ex_name}}_create(
        &self,
        request: Request<{{ProtoExName}}CreateRequest>,
    ) -> Result<Response<{{ProtoExName}}CreateResponse>, Status> {
        todo!()
    }

    async fn {{proto_ex_name}}_search(
        &self,
        request: Request<{{ProtoExName}}SearchRequest>,
    ) -> Result<Response<{{ProtoExName}}SearchResponse>, Status> {
        todo!()
    }
}
