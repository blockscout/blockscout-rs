#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTransactionsTransactionHashLogsResponse {
    pub items: Vec<crate::log::Log>,
    pub next_page_params: crate::get_transactions_transaction_hash_logs_response::GetTransactionsTransactionHashLogsResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTransactionsTransactionHashLogsResponseNextPageParams {}

impl GetTransactionsTransactionHashLogsResponse {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetTransactionsTransactionHashLogsResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams> {
        GetTransactionsTransactionHashLogsResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_logs() -> GetTransactionsTransactionHashLogsResponseGetBuilder<crate::generics::MissingTransactionHash> {
        GetTransactionsTransactionHashLogsResponseGetBuilder {
            inner: Default::default(),
            _param_transaction_hash: core::marker::PhantomData,
        }
    }
}

impl Into<GetTransactionsTransactionHashLogsResponse> for GetTransactionsTransactionHashLogsResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists> {
    fn into(self) -> GetTransactionsTransactionHashLogsResponse {
        self.body
    }
}

/// Builder for [`GetTransactionsTransactionHashLogsResponse`](./struct.GetTransactionsTransactionHashLogsResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetTransactionsTransactionHashLogsResponseBuilder<Items, NextPageParams> {
    body: self::GetTransactionsTransactionHashLogsResponse,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams> GetTransactionsTransactionHashLogsResponseBuilder<Items, NextPageParams> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::log::LogBuilder<crate::generics::AddressExists, crate::generics::DataExists, crate::generics::IndexExists, crate::generics::TopicsExists, crate::generics::TxHashExists>>) -> GetTransactionsTransactionHashLogsResponseBuilder<crate::generics::ItemsExists, NextPageParams> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_transactions_transaction_hash_logs_response::GetTransactionsTransactionHashLogsResponseNextPageParams) -> GetTransactionsTransactionHashLogsResponseBuilder<Items, crate::generics::NextPageParamsExists> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetTransactionsTransactionHashLogsResponse::get_logs`](./struct.GetTransactionsTransactionHashLogsResponse.html#method.get_logs) method for a `GET` operation associated with `GetTransactionsTransactionHashLogsResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetTransactionsTransactionHashLogsResponseGetBuilder<TransactionHash> {
    inner: GetTransactionsTransactionHashLogsResponseGetBuilderContainer,
    _param_transaction_hash: core::marker::PhantomData<TransactionHash>,
}

#[derive(Debug, Default, Clone)]
struct GetTransactionsTransactionHashLogsResponseGetBuilderContainer {
    param_transaction_hash: Option<String>,
}

impl<TransactionHash> GetTransactionsTransactionHashLogsResponseGetBuilder<TransactionHash> {
    /// Transaction hash
    #[inline]
    pub fn transaction_hash(mut self, value: impl Into<String>) -> GetTransactionsTransactionHashLogsResponseGetBuilder<crate::generics::TransactionHashExists> {
        self.inner.param_transaction_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetTransactionsTransactionHashLogsResponseGetBuilder<crate::generics::TransactionHashExists> {
    type Output = GetTransactionsTransactionHashLogsResponse;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/transactions/{transaction_hash}/logs", transaction_hash=self.inner.param_transaction_hash.as_ref().expect("missing parameter transaction_hash?")).into()
    }
}

