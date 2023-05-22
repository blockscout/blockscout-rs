#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTransactionsTransactionHashInternalTransactionsResponse {
    pub items: Vec<crate::internal_transaction::InternalTransaction>,
    pub next_page_params: crate::get_transactions_transaction_hash_internal_transactions_response::GetTransactionsTransactionHashInternalTransactionsResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTransactionsTransactionHashInternalTransactionsResponseNextPageParams {}

impl GetTransactionsTransactionHashInternalTransactionsResponse {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetTransactionsTransactionHashInternalTransactionsResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams> {
        GetTransactionsTransactionHashInternalTransactionsResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_internal_txs() -> GetTransactionsTransactionHashInternalTransactionsResponseGetBuilder<crate::generics::MissingTransactionHash> {
        GetTransactionsTransactionHashInternalTransactionsResponseGetBuilder {
            inner: Default::default(),
            _param_transaction_hash: core::marker::PhantomData,
        }
    }
}

impl Into<GetTransactionsTransactionHashInternalTransactionsResponse> for GetTransactionsTransactionHashInternalTransactionsResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists> {
    fn into(self) -> GetTransactionsTransactionHashInternalTransactionsResponse {
        self.body
    }
}

/// Builder for [`GetTransactionsTransactionHashInternalTransactionsResponse`](./struct.GetTransactionsTransactionHashInternalTransactionsResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetTransactionsTransactionHashInternalTransactionsResponseBuilder<Items, NextPageParams> {
    body: self::GetTransactionsTransactionHashInternalTransactionsResponse,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams> GetTransactionsTransactionHashInternalTransactionsResponseBuilder<Items, NextPageParams> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::internal_transaction::InternalTransactionBuilder<crate::generics::BlockExists, crate::generics::CreatedContractExists, crate::generics::FromExists, crate::generics::IndexExists, crate::generics::SuccessExists, crate::generics::TimestampExists, crate::generics::ToExists, crate::generics::TransactionHashExists, crate::generics::TypeExists, crate::generics::ValueExists>>) -> GetTransactionsTransactionHashInternalTransactionsResponseBuilder<crate::generics::ItemsExists, NextPageParams> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_transactions_transaction_hash_internal_transactions_response::GetTransactionsTransactionHashInternalTransactionsResponseNextPageParams) -> GetTransactionsTransactionHashInternalTransactionsResponseBuilder<Items, crate::generics::NextPageParamsExists> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetTransactionsTransactionHashInternalTransactionsResponse::get_internal_txs`](./struct.GetTransactionsTransactionHashInternalTransactionsResponse.html#method.get_internal_txs) method for a `GET` operation associated with `GetTransactionsTransactionHashInternalTransactionsResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetTransactionsTransactionHashInternalTransactionsResponseGetBuilder<TransactionHash> {
    inner: GetTransactionsTransactionHashInternalTransactionsResponseGetBuilderContainer,
    _param_transaction_hash: core::marker::PhantomData<TransactionHash>,
}

#[derive(Debug, Default, Clone)]
struct GetTransactionsTransactionHashInternalTransactionsResponseGetBuilderContainer {
    param_transaction_hash: Option<String>,
}

impl<TransactionHash> GetTransactionsTransactionHashInternalTransactionsResponseGetBuilder<TransactionHash> {
    /// Transaction hash
    #[inline]
    pub fn transaction_hash(mut self, value: impl Into<String>) -> GetTransactionsTransactionHashInternalTransactionsResponseGetBuilder<crate::generics::TransactionHashExists> {
        self.inner.param_transaction_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetTransactionsTransactionHashInternalTransactionsResponseGetBuilder<crate::generics::TransactionHashExists> {
    type Output = GetTransactionsTransactionHashInternalTransactionsResponse;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/transactions/{transaction_hash}/internal-transactions", transaction_hash=self.inner.param_transaction_hash.as_ref().expect("missing parameter transaction_hash?")).into()
    }
}

