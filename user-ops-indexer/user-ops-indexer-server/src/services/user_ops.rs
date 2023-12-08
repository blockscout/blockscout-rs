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

macro_rules! parse_filter {
    ( Option<$t:ident>, $x:expr ) => {
        match $x {
            Some(a) => Some(parse_filter!($t, a)),
            None => None,
        }
    };
    ( $t:ident, $x:expr ) => {
        $t::from_str(&$x)
            .map_err(|e| Status::invalid_argument(format!("Invalid {}: {e}", stringify!($x))))?
    };
}

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

        let address = parse_filter!(Address, inner.address);

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

        let op_hash = parse_filter!(H256, inner.op_hash);

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

        let factory = parse_filter!(Address, inner.address);

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

        let factory_filter = parse_filter!(Option<Address>, inner.factory);
        let page_token = parse_filter!(Option<Address>, inner.page_token);
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

        let bundler_filter = parse_filter!(Option<Address>, inner.bundler);
        let entry_point_filter = parse_filter!(Option<Address>, inner.entry_point);
        let page_token = if let Some(page_token) = inner.page_token {
            match page_token.split(',').collect::<Vec<&str>>().as_slice() {
                [page_token_block_number, page_token_tx_hash, page_token_bundle_index] => {
                    Ok(Some((
                        parse_filter!(u64, page_token_block_number),
                        parse_filter!(H256, page_token_tx_hash),
                        parse_filter!(u64, page_token_bundle_index),
                    )))
                }
                _ => Err(Status::invalid_argument("invalid page_token format")),
            }
        } else {
            Ok(None)
        }?;
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

        let sender_filter = parse_filter!(Option<Address>, inner.sender);
        let bundler_filter = parse_filter!(Option<Address>, inner.bundler);
        let paymaster_filter = parse_filter!(Option<Address>, inner.paymaster);
        let factory_filter = parse_filter!(Option<Address>, inner.factory);
        let tx_hash_filter = parse_filter!(Option<H256>, inner.tx_hash);
        let entry_point_filter = parse_filter!(Option<Address>, inner.entry_point);
        let bundle_index_filter = inner.bundle_index;
        let block_number_filter = inner.block_number;
        let page_token = if let Some(page_token) = inner.page_token {
            match page_token.split(',').collect::<Vec<&str>>().as_slice() {
                [page_token_block_number, page_token_op_hash] => Ok(Some((
                    parse_filter!(u64, page_token_block_number),
                    parse_filter!(H256, page_token_op_hash),
                ))),
                _ => Err(Status::invalid_argument("invalid page_token format")),
            }
        } else {
            Ok(None)
        }?;
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

        let page_token = if let Some(page_token) = inner.page_token {
            match page_token.split(',').collect::<Vec<&str>>().as_slice() {
                [page_token_total_accounts, page_token_factory] => Ok(Some((
                    parse_filter!(u64, page_token_total_accounts),
                    parse_filter!(Address, page_token_factory),
                ))),
                _ => Err(Status::invalid_argument("invalid page_token format")),
            }
        } else {
            Ok(None)
        }?;
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
