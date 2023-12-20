use crate::{proto::user_ops_service_server::UserOpsService as UserOps, settings::ApiSettings};
use ethers::{
    abi::{AbiEncode, Address},
    prelude::H256,
    utils::to_checksum,
};
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use tonic::{Request, Response, Status};
use user_ops_indexer_logic::repository;
use user_ops_indexer_proto::blockscout::user_ops_indexer::v1::{
    Account, Bundler, Factory, GetAccountRequest, GetBundlerRequest, GetFactoryRequest,
    GetPaymasterRequest, GetUserOpRequest, ListAccountsRequest, ListAccountsResponse,
    ListBundlersRequest, ListBundlersResponse, ListBundlesRequest, ListBundlesResponse,
    ListFactoriesRequest, ListFactoriesResponse, ListPaymastersRequest, ListPaymastersResponse,
    ListUserOpsRequest, ListUserOpsResponse, Paymaster, UserOp,
};

const DEFAULT_PAGE_SIZE: u32 = 10;

pub struct UserOpsService {
    db: DatabaseConnection,

    settings: ApiSettings,
}

impl UserOpsService {
    pub fn new(db: DatabaseConnection, settings: ApiSettings) -> Self {
        Self { db, settings }
    }

    fn normalize_page_size(&self, size: Option<u32>) -> u64 {
        size.unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, self.settings.max_page_size) as u64
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

        let op_hash = parse_filter(inner.hash)?;

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
        request: Request<GetBundlerRequest>,
    ) -> Result<Response<Bundler>, Status> {
        let inner = request.into_inner();

        let bundler = parse_filter(inner.address)?;

        let bundler = repository::bundler::find_bundler_by_address(&self.db, bundler)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to query bundler");
                Status::internal("failed to query bundler")
            })?
            .ok_or(Status::not_found("bundler not found"))?;

        Ok(Response::new(bundler.into()))
    }

    async fn get_paymaster(
        &self,
        request: Request<GetPaymasterRequest>,
    ) -> Result<Response<Paymaster>, Status> {
        let inner = request.into_inner();

        let paymaster = parse_filter(inner.address)?;

        let paymaster = repository::paymaster::find_paymaster_by_address(&self.db, paymaster)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to query paymaster");
                Status::internal("failed to query paymaster")
            })?
            .ok_or(Status::not_found("paymaster not found"))?;

        Ok(Response::new(paymaster.into()))
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
        let page_size = self.normalize_page_size(inner.page_size);

        let (accounts, next_page_token) =
            repository::account::list_accounts(&self.db, factory_filter, page_token, page_size)
                .await
                .map_err(|err| {
                    tracing::error!(error = ?err, "failed to query accounts");
                    Status::internal("failed to query accounts")
                })?;

        let res = ListAccountsResponse {
            items: accounts.into_iter().map(|acc| acc.into()).collect(),
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

        let page_token: Option<(u64, H256, u64)> =
            inner.page_token.map(parse_filter_3).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);

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
            items: bundles.into_iter().map(|b| b.into()).collect(),
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
        let tx_hash_filter = inner.transaction_hash.map(parse_filter).transpose()?;
        let entry_point_filter = inner.entry_point.map(parse_filter).transpose()?;
        let bundle_index_filter = inner.bundle_index;
        let block_number_filter = inner.block_number;

        let page_token: Option<(u64, H256)> = inner.page_token.map(parse_filter_2).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);

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
            items: ops.into_iter().map(|acc| acc.into()).collect(),
            next_page_token: next_page_token.map(|(b, o)| format!("{},{}", b, o.encode_hex())),
        };

        Ok(Response::new(res))
    }

    async fn list_bundlers(
        &self,
        request: Request<ListBundlersRequest>,
    ) -> Result<Response<ListBundlersResponse>, Status> {
        let inner = request.into_inner();

        let page_token: Option<(u64, Address)> =
            inner.page_token.map(parse_filter_2).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);

        let (bundlers, next_page_token) =
            repository::bundler::list_bundlers(&self.db, page_token, page_size)
                .await
                .map_err(|err| {
                    tracing::error!(error = ?err, "failed to query bundlers");
                    Status::internal("failed to query bundlers")
                })?;

        let res = ListBundlersResponse {
            items: bundlers.into_iter().map(|b| b.into()).collect(),
            next_page_token: next_page_token
                .map(|(t, f)| format!("{},{}", t, to_checksum(&f, None))),
        };

        Ok(Response::new(res))
    }

    async fn list_paymasters(
        &self,
        request: Request<ListPaymastersRequest>,
    ) -> Result<Response<ListPaymastersResponse>, Status> {
        let inner = request.into_inner();

        let page_token: Option<(u64, Address)> =
            inner.page_token.map(parse_filter_2).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);

        let (paymasters, next_page_token) =
            repository::paymaster::list_paymasters(&self.db, page_token, page_size)
                .await
                .map_err(|err| {
                    tracing::error!(error = ?err, "failed to query paymasters");
                    Status::internal("failed to query paymasters")
                })?;

        let res = ListPaymastersResponse {
            items: paymasters.into_iter().map(|b| b.into()).collect(),
            next_page_token: next_page_token
                .map(|(t, f)| format!("{},{}", t, to_checksum(&f, None))),
        };

        Ok(Response::new(res))
    }

    async fn list_factories(
        &self,
        request: Request<ListFactoriesRequest>,
    ) -> Result<Response<ListFactoriesResponse>, Status> {
        let inner = request.into_inner();

        let page_token: Option<(u64, Address)> =
            inner.page_token.map(parse_filter_2).transpose()?;
        let page_size = self.normalize_page_size(inner.page_size);

        let (factories, next_page_token) =
            repository::factory::list_factories(&self.db, page_token, page_size)
                .await
                .map_err(|err| {
                    tracing::error!(error = ?err, "failed to query factories");
                    Status::internal("failed to query factories")
                })?;

        let res = ListFactoriesResponse {
            items: factories.into_iter().map(|b| b.into()).collect(),
            next_page_token: next_page_token
                .map(|(t, f)| format!("{},{}", t, to_checksum(&f, None))),
        };

        Ok(Response::new(res))
    }
}

#[inline]
fn parse_filter<T: FromStr>(input: String) -> Result<T, Status>
where
    <T as FromStr>::Err: std::fmt::Display,
{
    T::from_str(&input)
        .map_err(|e| Status::invalid_argument(format!("Invalid value {}: {e}", input)))
}

#[inline]
fn parse_filter_2<T1: FromStr, T2: FromStr>(input: String) -> Result<(T1, T2), Status>
where
    <T1 as FromStr>::Err: std::fmt::Display,
    <T2 as FromStr>::Err: std::fmt::Display,
{
    match input.split(',').collect::<Vec<&str>>().as_slice() {
        [v1, v2] => Ok((
            parse_filter::<T1>(v1.to_string())?,
            parse_filter::<T2>(v2.to_string())?,
        )),
        _ => Err(Status::invalid_argument("invalid page_token format")),
    }
}

#[inline]
fn parse_filter_3<T1: FromStr, T2: FromStr, T3: FromStr>(
    input: String,
) -> Result<(T1, T2, T3), Status>
where
    <T1 as FromStr>::Err: std::fmt::Display,
    <T2 as FromStr>::Err: std::fmt::Display,
    <T3 as FromStr>::Err: std::fmt::Display,
{
    match input.split(',').collect::<Vec<&str>>().as_slice() {
        [v1, v2, v3] => Ok((
            parse_filter::<T1>(v1.to_string())?,
            parse_filter::<T2>(v2.to_string())?,
            parse_filter::<T3>(v3.to_string())?,
        )),
        _ => Err(Status::invalid_argument("invalid page_token format")),
    }
}
