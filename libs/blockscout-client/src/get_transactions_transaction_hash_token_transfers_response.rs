#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTransactionsTransactionHashTokenTransfersResponse<Any> {
    pub items: Vec<crate::token_transfer::TokenTransfer<Any>>,
    pub next_page_params: crate::get_transactions_transaction_hash_token_transfers_response::GetTransactionsTransactionHashTokenTransfersResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTransactionsTransactionHashTokenTransfersResponseNextPageParams {}

impl<Any: Default> GetTransactionsTransactionHashTokenTransfersResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetTransactionsTransactionHashTokenTransfersResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams, Any> {
        GetTransactionsTransactionHashTokenTransfersResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_token_transfers() -> GetTransactionsTransactionHashTokenTransfersResponseGetBuilder<crate::generics::MissingTransactionHash> {
        GetTransactionsTransactionHashTokenTransfersResponseGetBuilder {
            inner: Default::default(),
            _param_transaction_hash: core::marker::PhantomData,
        }
    }
}

impl<Any> Into<GetTransactionsTransactionHashTokenTransfersResponse<Any>> for GetTransactionsTransactionHashTokenTransfersResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists, Any> {
    fn into(self) -> GetTransactionsTransactionHashTokenTransfersResponse<Any> {
        self.body
    }
}

/// Builder for [`GetTransactionsTransactionHashTokenTransfersResponse`](./struct.GetTransactionsTransactionHashTokenTransfersResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetTransactionsTransactionHashTokenTransfersResponseBuilder<Items, NextPageParams, Any> {
    body: self::GetTransactionsTransactionHashTokenTransfersResponse<Any>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams, Any> GetTransactionsTransactionHashTokenTransfersResponseBuilder<Items, NextPageParams, Any> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::token_transfer::TokenTransferBuilder<crate::generics::FromExists, crate::generics::ToExists, crate::generics::TokenExists, crate::generics::TotalExists, crate::generics::TxHashExists, crate::generics::TypeExists, Any>>) -> GetTransactionsTransactionHashTokenTransfersResponseBuilder<crate::generics::ItemsExists, NextPageParams, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_transactions_transaction_hash_token_transfers_response::GetTransactionsTransactionHashTokenTransfersResponseNextPageParams) -> GetTransactionsTransactionHashTokenTransfersResponseBuilder<Items, crate::generics::NextPageParamsExists, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetTransactionsTransactionHashTokenTransfersResponse::get_token_transfers`](./struct.GetTransactionsTransactionHashTokenTransfersResponse.html#method.get_token_transfers) method for a `GET` operation associated with `GetTransactionsTransactionHashTokenTransfersResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetTransactionsTransactionHashTokenTransfersResponseGetBuilder<TransactionHash> {
    inner: GetTransactionsTransactionHashTokenTransfersResponseGetBuilderContainer,
    _param_transaction_hash: core::marker::PhantomData<TransactionHash>,
}

#[derive(Debug, Default, Clone)]
struct GetTransactionsTransactionHashTokenTransfersResponseGetBuilderContainer {
    param_transaction_hash: Option<String>,
    param_type: Option<String>,
}

impl<TransactionHash> GetTransactionsTransactionHashTokenTransfersResponseGetBuilder<TransactionHash> {
    /// Transaction hash
    #[inline]
    pub fn transaction_hash(mut self, value: impl Into<String>) -> GetTransactionsTransactionHashTokenTransfersResponseGetBuilder<crate::generics::TransactionHashExists> {
        self.inner.param_transaction_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> Self {
        self.inner.param_type = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetTransactionsTransactionHashTokenTransfersResponseGetBuilder<crate::generics::TransactionHashExists> {
    type Output = GetTransactionsTransactionHashTokenTransfersResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/transactions/{transaction_hash}/token-transfers", transaction_hash=self.inner.param_transaction_hash.as_ref().expect("missing parameter transaction_hash?")).into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .header(http::header::ACCEPT.as_str(), "application/yaml")
        .query(&[
            ("type", self.inner.param_type.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

