#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetBlocksBlockNumberOrHashTransactionsResponse<Any> {
    pub items: Vec<crate::transaction::Transaction<Any>>,
    pub next_page_params: crate::get_blocks_block_number_or_hash_transactions_response::GetBlocksBlockNumberOrHashTransactionsResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetBlocksBlockNumberOrHashTransactionsResponseNextPageParams {}

impl<Any: Default> GetBlocksBlockNumberOrHashTransactionsResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetBlocksBlockNumberOrHashTransactionsResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams, Any> {
        GetBlocksBlockNumberOrHashTransactionsResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_block_txs() -> GetBlocksBlockNumberOrHashTransactionsResponseGetBuilder<crate::generics::MissingBlockNumberOrHash> {
        GetBlocksBlockNumberOrHashTransactionsResponseGetBuilder {
            inner: Default::default(),
            _param_block_number_or_hash: core::marker::PhantomData,
        }
    }
}

impl<Any> Into<GetBlocksBlockNumberOrHashTransactionsResponse<Any>> for GetBlocksBlockNumberOrHashTransactionsResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists, Any> {
    fn into(self) -> GetBlocksBlockNumberOrHashTransactionsResponse<Any> {
        self.body
    }
}

/// Builder for [`GetBlocksBlockNumberOrHashTransactionsResponse`](./struct.GetBlocksBlockNumberOrHashTransactionsResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetBlocksBlockNumberOrHashTransactionsResponseBuilder<Items, NextPageParams, Any> {
    body: self::GetBlocksBlockNumberOrHashTransactionsResponse<Any>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams, Any> GetBlocksBlockNumberOrHashTransactionsResponseBuilder<Items, NextPageParams, Any> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::transaction::TransactionBuilder<crate::generics::ConfirmationDurationExists, crate::generics::ConfirmationsExists, crate::generics::CreatedContractExists, crate::generics::FeeExists, crate::generics::FromExists, crate::generics::GasLimitExists, crate::generics::GasPriceExists, crate::generics::HashExists, crate::generics::NonceExists, crate::generics::RawInputExists, crate::generics::ResultExists, crate::generics::ToExists, crate::generics::TxTypesExists, crate::generics::ValueExists, Any>>) -> GetBlocksBlockNumberOrHashTransactionsResponseBuilder<crate::generics::ItemsExists, NextPageParams, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_blocks_block_number_or_hash_transactions_response::GetBlocksBlockNumberOrHashTransactionsResponseNextPageParams) -> GetBlocksBlockNumberOrHashTransactionsResponseBuilder<Items, crate::generics::NextPageParamsExists, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetBlocksBlockNumberOrHashTransactionsResponse::get_block_txs`](./struct.GetBlocksBlockNumberOrHashTransactionsResponse.html#method.get_block_txs) method for a `GET` operation associated with `GetBlocksBlockNumberOrHashTransactionsResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetBlocksBlockNumberOrHashTransactionsResponseGetBuilder<BlockNumberOrHash> {
    inner: GetBlocksBlockNumberOrHashTransactionsResponseGetBuilderContainer,
    _param_block_number_or_hash: core::marker::PhantomData<BlockNumberOrHash>,
}

#[derive(Debug, Default, Clone)]
struct GetBlocksBlockNumberOrHashTransactionsResponseGetBuilderContainer {
    param_block_number_or_hash: Option<String>,
}

impl<BlockNumberOrHash> GetBlocksBlockNumberOrHashTransactionsResponseGetBuilder<BlockNumberOrHash> {
    /// Block number or hash
    #[inline]
    pub fn block_number_or_hash(mut self, value: impl Into<String>) -> GetBlocksBlockNumberOrHashTransactionsResponseGetBuilder<crate::generics::BlockNumberOrHashExists> {
        self.inner.param_block_number_or_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetBlocksBlockNumberOrHashTransactionsResponseGetBuilder<crate::generics::BlockNumberOrHashExists> {
    type Output = GetBlocksBlockNumberOrHashTransactionsResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/blocks/{block_number_or_hash}/transactions", block_number_or_hash=self.inner.param_block_number_or_hash.as_ref().expect("missing parameter block_number_or_hash?")).into()
    }
}

