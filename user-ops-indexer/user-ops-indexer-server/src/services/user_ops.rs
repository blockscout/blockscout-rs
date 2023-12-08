use std::str::FromStr;

use ethers::abi::{AbiEncode, Address};
use ethers::prelude::H256;
use ethers::utils::to_checksum;
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

use user_ops_indexer::repository;
use user_ops_indexer_proto::blockscout::user_ops_indexer::v1::{
    Account, Bundler, Factory, GetAccountRequest, GetBundlerRequest, GetFactoryRequest,
    GetPaymasterRequest, GetUserOpRequest, ListAccountsRequest, ListAccountsResponse,
    ListBundlersRequest, ListBundlersResponse, ListBundlesRequest, ListBundlesResponse,
    ListFactoriesRequest, ListFactoriesResponse, ListPaymastersRequest, ListPaymastersResponse,
    ListUserOpsRequest, ListUserOpsResponse, Paymaster, UserOp,
};

use crate::proto::user_ops_service_server::UserOpsService as UserOps;

const DEFAULT_PAGE_SIZE: u64 = 10;
const MAX_PAGE_SIZE: u64 = 100;

#[derive(Default)]
pub struct UserOpsService {
    db: DatabaseConnection,
}

impl UserOpsService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl UserOps for UserOpsService {
    async fn get_account(
        &self,
        request: Request<GetAccountRequest>,
    ) -> Result<Response<Account>, Status> {
        let inner = request.into_inner();

        let address = parse_filter(inner.address)?;

        let acc = repository::account::find_account_by_address(&self.db, address)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to query account");
                Status::internal("failed to query account")
            })?
            .ok_or(Status::not_found("account not found"))?;

        Ok(Response::new(acc.into()))
    }

    async fn get_user_op(
        &self,
        request: Request<GetUserOpRequest>,
    ) -> Result<Response<UserOp>, Status> {
        let inner = request.into_inner();

        let op_hash = parse_filter(inner.op_hash)?;

        let user_op = repository::user_op::find_user_op_by_op_hash(&self.db, op_hash)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to query user operation");
                Status::internal("failed to query user operation")
            })?
            .ok_or(Status::not_found("user operation not found"))?;

        Ok(Response::new(user_op.into()))
    }

    async fn get_bundler(
        &self,
        _request: Request<GetBundlerRequest>,
    ) -> Result<Response<Bundler>, Status> {
        todo!()
    }

    async fn get_paymaster(
        &self,
        _request: Request<GetPaymasterRequest>,
    ) -> Result<Response<Paymaster>, Status> {
        todo!()
    }

    async fn get_factory(
        &self,
        request: Request<GetFactoryRequest>,
    ) -> Result<Response<Factory>, Status> {
        let inner = request.into_inner();

        let factory = parse_filter(inner.address)?;

        let factory = repository::factory::find_factory_by_address(&self.db, factory)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to query factory");
                Status::internal("failed to query factory")
            })?
            .ok_or(Status::not_found("factory not found"))?;

        Ok(Response::new(factory.into()))
    }

    async fn list_accounts(
        &self,
        request: Request<ListAccountsRequest>,
    ) -> Result<Response<ListAccountsResponse>, Status> {
        let inner = request.into_inner();

        let factory_filter = inner.factory.map(parse_filter).transpose()?;
        let page_token = inner.page_token.map(parse_filter).transpose()?;
        let page_size = normalize_page_size(inner.page_size);

        let (accounts, next_page_token) =
            repository::account::list_accounts(&self.db, factory_filter, page_token, page_size)
                .await
                .map_err(|err| {
                    tracing::error!(error = ?err, "failed to query accounts");
                    Status::internal("failed to query accounts")
                })?;

        let res = ListAccountsResponse {
            accounts: accounts.into_iter().map(|acc| acc.into()).collect(),
            next_page_token: next_page_token.map(|a| to_checksum(&a, None)),
        };

        Ok(Response::new(res))
    }

    async fn list_bundles(
        &self,
        request: Request<ListBundlesRequest>,
    ) -> Result<Response<ListBundlesResponse>, Status> {
        let inner = request.into_inner();

        let bundler_filter = inner.bundler.map(parse_filter).transpose()?;
        let entry_point_filter = inner.entry_point.map(parse_filter).transpose()?;

        let page_token = inner
            .page_token
            .map(|t| match t.split(',').collect::<Vec<&str>>().as_slice() {
                [page_token_block_number, page_token_tx_hash, page_token_bundle_index] => Ok((
                    parse_filter::<u64>(page_token_block_number.to_string())?,
                    parse_filter::<H256>(page_token_tx_hash.to_string())?,
                    parse_filter::<u64>(page_token_bundle_index.to_string())?,
                )),
                _ => Err(Status::invalid_argument("invalid page_token format")),
            })
            .transpose()?;
        let page_size = normalize_page_size(inner.page_size);

        let (bundles, next_page_token) = repository::bundle::list_bundles(
            &self.db,
            bundler_filter,
            entry_point_filter,
            page_token,
            page_size,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to query bundles");
            Status::internal("failed to query bundles")
        })?;

        let res = ListBundlesResponse {
            bundles: bundles.into_iter().map(|b| b.into()).collect(),
            next_page_token: next_page_token
                .map(|(b, t, i)| format!("{},{},{}", b, t.encode_hex(), i)),
        };

        Ok(Response::new(res))
    }

    async fn list_user_ops(
        &self,
        request: Request<ListUserOpsRequest>,
    ) -> Result<Response<ListUserOpsResponse>, Status> {
        let inner = request.into_inner();

        let sender_filter = inner.sender.map(parse_filter).transpose()?;
        let bundler_filter = inner.bundler.map(parse_filter).transpose()?;
        let paymaster_filter = inner.paymaster.map(parse_filter).transpose()?;
        let factory_filter = inner.factory.map(parse_filter).transpose()?;
        let tx_hash_filter = inner.tx_hash.map(parse_filter).transpose()?;
        let entry_point_filter = inner.entry_point.map(parse_filter).transpose()?;
        let bundle_index_filter = inner.bundle_index;
        let block_number_filter = inner.block_number;

        let page_token = inner
            .page_token
            .map(|t| match t.split(',').collect::<Vec<&str>>().as_slice() {
                [page_token_block_number, page_token_op_hash] => Ok((
                    parse_filter::<u64>(page_token_block_number.to_string())?,
                    parse_filter::<H256>(page_token_op_hash.to_string())?,
                )),
                _ => Err(Status::invalid_argument("invalid page_token format")),
            })
            .transpose()?;

        let page_size = normalize_page_size(inner.page_size);

        let (ops, next_page_token) = repository::user_op::list_user_ops(
            &self.db,
            sender_filter,
            bundler_filter,
            paymaster_filter,
            factory_filter,
            tx_hash_filter,
            entry_point_filter,
            bundle_index_filter,
            block_number_filter,
            page_token,
            page_size,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to query user operations");
            Status::internal("failed to query user operations")
        })?;

        let res = ListUserOpsResponse {
            ops: ops.into_iter().map(|acc| acc.into()).collect(),
            next_page_token: next_page_token.map(|(b, o)| format!("{},{}", b, o.encode_hex())),
        };

        Ok(Response::new(res))
    }

    async fn list_bundlers(
        &self,
        _request: Request<ListBundlersRequest>,
    ) -> Result<Response<ListBundlersResponse>, Status> {
        todo!()
    }

    async fn list_paymasters(
        &self,
        _request: Request<ListPaymastersRequest>,
    ) -> Result<Response<ListPaymastersResponse>, Status> {
        todo!()
    }

    async fn list_factories(
        &self,
        request: Request<ListFactoriesRequest>,
    ) -> Result<Response<ListFactoriesResponse>, Status> {
        let inner = request.into_inner();

        let page_token = inner
            .page_token
            .map(|t| match t.split(',').collect::<Vec<&str>>().as_slice() {
                [page_token_total_accounts, page_token_factory] => Ok((
                    parse_filter::<u64>(page_token_total_accounts.to_string())?,
                    parse_filter::<Address>(page_token_factory.to_string())?,
                )),
                _ => Err(Status::invalid_argument("invalid page_token format")),
            })
            .transpose()?;
        let page_size = normalize_page_size(inner.page_size);

        let (factories, next_page_token) =
            repository::factory::list_factories(&self.db, page_token, page_size)
                .await
                .map_err(|err| {
                    tracing::error!(error = ?err, "failed to query factories");
                    Status::internal("failed to query factories")
                })?;

        let res = ListFactoriesResponse {
            factories: factories.into_iter().map(|b| b.into()).collect(),
            next_page_token: next_page_token
                .map(|(t, f)| format!("{},{}", t, to_checksum(&f, None))),
        };

        Ok(Response::new(res))
    }
}

fn normalize_page_size(size: Option<u32>) -> u64 {
    size.map_or(DEFAULT_PAGE_SIZE, |a| a as u64)
        .clamp(1, MAX_PAGE_SIZE)
}

#[inline]
fn parse_filter<T: FromStr>(input: String) -> Result<T, Status>
where
    <T as FromStr>::Err: std::fmt::Display,
{
    T::from_str(&input).map_err(|e| Status::invalid_argument(format!("Invalid value {}: {e}", input)))
}
