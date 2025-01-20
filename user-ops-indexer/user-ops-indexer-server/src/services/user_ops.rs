use crate::{proto::user_ops_service_server::UserOpsService as UserOps, settings::ApiSettings};
use sea_orm::DatabaseConnection;
use std::str::FromStr;
use tonic::{Request, Response, Status};
use user_ops_indexer_logic::{
    repository,
    repository::page_token::{PageTokenFormat, PageTokenParsingError},
};
use user_ops_indexer_proto::blockscout::user_ops_indexer::v1::*;

const DEFAULT_PAGE_SIZE: u32 = 50;

enum UserOpsError {
    PageTokenError(String, PageTokenParsingError),
    FilterError(String, String),
}

impl From<UserOpsError> for Status {
    fn from(value: UserOpsError) -> Self {
        match value {
            UserOpsError::PageTokenError(v, err) => {
                Status::invalid_argument(format!("invalid format '{v}' for page_token: {err}"))
            }
            UserOpsError::FilterError(v, field) => {
                Status::invalid_argument(format!("invalid format '{v}' for filter {field}"))
            }
        }
    }
}

pub struct UserOpsService {
    db: DatabaseConnection,

    settings: ApiSettings,
}

impl UserOpsService {
    pub fn new(db: DatabaseConnection, settings: ApiSettings) -> Self {
        Self { db, settings }
    }

    fn normalize_page_size(&self, size: Option<u32>) -> u32 {
        size.unwrap_or(DEFAULT_PAGE_SIZE)
            .clamp(1, self.settings.max_page_size)
    }
}

#[async_trait::async_trait]
impl UserOps for UserOpsService {
    async fn get_account(
        &self,
        request: Request<GetAccountRequest>,
    ) -> Result<Response<Account>, Status> {
        let inner = request.into_inner();

        let address = inner.address.parse_filter("address")?;

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

        let op_hash = inner.hash.parse_filter("hash")?;

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

        let bundler = inner.address.parse_filter("address")?;

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

        let paymaster = inner.address.parse_filter("address")?;

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

        let factory = inner.address.parse_filter("address")?;

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

        let factory_filter = inner.factory.parse_filter("factory")?;
        let page_token = inner.page_token.parse_page_token()?;
        let page_size = self.normalize_page_size(inner.page_size);

        let (accounts, next_page_token) = repository::account::list_accounts(
            &self.db,
            factory_filter,
            page_token,
            page_size as u64,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to query accounts");
            Status::internal("failed to query accounts")
        })?;

        let res = ListAccountsResponse {
            items: accounts.into_iter().map(|acc| acc.into()).collect(),
            next_page_params: page_token_to_proto(next_page_token, page_size),
        };

        Ok(Response::new(res))
    }

    async fn list_bundles(
        &self,
        request: Request<ListBundlesRequest>,
    ) -> Result<Response<ListBundlesResponse>, Status> {
        let inner = request.into_inner();

        let bundler_filter = inner.bundler.parse_filter("bundler")?;
        let entry_point_filter = inner.entry_point.parse_filter("entry_point")?;
        let page_token = inner.page_token.parse_page_token()?;
        let page_size = self.normalize_page_size(inner.page_size);

        let (bundles, next_page_token) = repository::bundle::list_bundles(
            &self.db,
            bundler_filter,
            entry_point_filter,
            page_token,
            page_size as u64,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to query bundles");
            Status::internal("failed to query bundles")
        })?;

        let res = ListBundlesResponse {
            items: bundles.into_iter().map(|b| b.into()).collect(),
            next_page_params: page_token_to_proto(next_page_token, page_size),
        };

        Ok(Response::new(res))
    }

    async fn list_user_ops(
        &self,
        request: Request<ListUserOpsRequest>,
    ) -> Result<Response<ListUserOpsResponse>, Status> {
        let inner = request.into_inner();

        let sender_filter = inner.sender.parse_filter("sender")?;
        let bundler_filter = inner.bundler.parse_filter("bundler")?;
        let paymaster_filter = inner.paymaster.parse_filter("paymaster")?;
        let factory_filter = inner.factory.parse_filter("factory")?;
        let tx_hash_filter = inner.transaction_hash.parse_filter("transaction_hash")?;
        let entry_point_filter = inner.entry_point.parse_filter("entry_point")?;
        let bundle_index_filter = inner.bundle_index;
        let block_number_filter = inner.block_number;
        let page_token = inner.page_token.parse_page_token()?;
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
            page_size as u64,
        )
        .await
        .map_err(|err| {
            tracing::error!(error = ?err, "failed to query user operations");
            Status::internal("failed to query user operations")
        })?;

        let res = ListUserOpsResponse {
            items: ops.into_iter().map(|acc| acc.into()).collect(),
            next_page_params: page_token_to_proto(next_page_token, page_size),
        };

        Ok(Response::new(res))
    }

    async fn list_bundlers(
        &self,
        request: Request<ListBundlersRequest>,
    ) -> Result<Response<ListBundlersResponse>, Status> {
        let inner = request.into_inner();

        let page_token = inner.page_token.parse_page_token()?;
        let page_size = self.normalize_page_size(inner.page_size);

        let (bundlers, next_page_token) =
            repository::bundler::list_bundlers(&self.db, page_token, page_size as u64)
                .await
                .map_err(|err| {
                    tracing::error!(error = ?err, "failed to query bundlers");
                    Status::internal("failed to query bundlers")
                })?;

        let res = ListBundlersResponse {
            items: bundlers.into_iter().map(|b| b.into()).collect(),
            next_page_params: page_token_to_proto(next_page_token, page_size),
        };

        Ok(Response::new(res))
    }

    async fn list_paymasters(
        &self,
        request: Request<ListPaymastersRequest>,
    ) -> Result<Response<ListPaymastersResponse>, Status> {
        let inner = request.into_inner();

        let page_token = inner.page_token.parse_page_token()?;
        let page_size = self.normalize_page_size(inner.page_size);

        let (paymasters, next_page_token) =
            repository::paymaster::list_paymasters(&self.db, page_token, page_size as u64)
                .await
                .map_err(|err| {
                    tracing::error!(error = ?err, "failed to query paymasters");
                    Status::internal("failed to query paymasters")
                })?;

        let res = ListPaymastersResponse {
            items: paymasters.into_iter().map(|b| b.into()).collect(),
            next_page_params: page_token_to_proto(next_page_token, page_size),
        };

        Ok(Response::new(res))
    }

    async fn list_factories(
        &self,
        request: Request<ListFactoriesRequest>,
    ) -> Result<Response<ListFactoriesResponse>, Status> {
        let inner = request.into_inner();

        let page_token = inner.page_token.parse_page_token()?;
        let page_size = self.normalize_page_size(inner.page_size);

        let (factories, next_page_token) =
            repository::factory::list_factories(&self.db, page_token, page_size as u64)
                .await
                .map_err(|err| {
                    tracing::error!(error = ?err, "failed to query factories");
                    Status::internal("failed to query factories")
                })?;

        let res = ListFactoriesResponse {
            items: factories.into_iter().map(|b| b.into()).collect(),
            next_page_params: page_token_to_proto(next_page_token, page_size),
        };

        Ok(Response::new(res))
    }
}

fn page_token_to_proto<T: PageTokenFormat>(
    page_token: Option<T>,
    page_size: u32,
) -> Option<Pagination> {
    page_token.map(|pt| Pagination {
        page_token: pt.format(),
        page_size,
    })
}

trait ParsePageToken<T: PageTokenFormat> {
    fn parse_page_token(self) -> Result<Option<T>, UserOpsError>;
}

impl<T: PageTokenFormat> ParsePageToken<T> for Option<String> {
    fn parse_page_token(self) -> Result<Option<T>, UserOpsError> {
        self.map(|s| T::from(s.clone()).map_err(|err| UserOpsError::PageTokenError(s, err)))
            .transpose()
    }
}

trait ParseFilter<T: FromStr> {
    type R;
    fn parse_filter(self, field: &str) -> Result<Self::R, UserOpsError>;
}

impl<T: FromStr> ParseFilter<T> for String {
    type R = T;

    fn parse_filter(self, field: &str) -> Result<Self::R, UserOpsError> {
        self.parse()
            .map_err(|_| UserOpsError::FilterError(self, field.to_string()))
    }
}

impl<T: FromStr> ParseFilter<T> for Option<String> {
    type R = Option<T>;

    fn parse_filter(self, field: &str) -> Result<Self::R, UserOpsError> {
        self.map(|s| {
            s.parse()
                .map_err(|_| UserOpsError::FilterError(s, field.to_string()))
        })
        .transpose()
    }
}
