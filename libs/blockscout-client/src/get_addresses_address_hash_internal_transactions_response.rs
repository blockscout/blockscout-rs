#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashInternalTransactionsResponse {
    pub items: Vec<crate::internal_transaction::InternalTransaction>,
    pub next_page_params: crate::get_addresses_address_hash_internal_transactions_response::GetAddressesAddressHashInternalTransactionsResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashInternalTransactionsResponseNextPageParams {}

impl GetAddressesAddressHashInternalTransactionsResponse {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetAddressesAddressHashInternalTransactionsResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams> {
        GetAddressesAddressHashInternalTransactionsResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_address_internal_txs() -> GetAddressesAddressHashInternalTransactionsResponseGetBuilder<crate::generics::MissingAddressHash> {
        GetAddressesAddressHashInternalTransactionsResponseGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl Into<GetAddressesAddressHashInternalTransactionsResponse> for GetAddressesAddressHashInternalTransactionsResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists> {
    fn into(self) -> GetAddressesAddressHashInternalTransactionsResponse {
        self.body
    }
}

/// Builder for [`GetAddressesAddressHashInternalTransactionsResponse`](./struct.GetAddressesAddressHashInternalTransactionsResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashInternalTransactionsResponseBuilder<Items, NextPageParams> {
    body: self::GetAddressesAddressHashInternalTransactionsResponse,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams> GetAddressesAddressHashInternalTransactionsResponseBuilder<Items, NextPageParams> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::internal_transaction::InternalTransactionBuilder<crate::generics::BlockExists, crate::generics::CreatedContractExists, crate::generics::FromExists, crate::generics::IndexExists, crate::generics::SuccessExists, crate::generics::TimestampExists, crate::generics::ToExists, crate::generics::TransactionHashExists, crate::generics::TypeExists, crate::generics::ValueExists>>) -> GetAddressesAddressHashInternalTransactionsResponseBuilder<crate::generics::ItemsExists, NextPageParams> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_addresses_address_hash_internal_transactions_response::GetAddressesAddressHashInternalTransactionsResponseNextPageParams) -> GetAddressesAddressHashInternalTransactionsResponseBuilder<Items, crate::generics::NextPageParamsExists> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetAddressesAddressHashInternalTransactionsResponse::get_address_internal_txs`](./struct.GetAddressesAddressHashInternalTransactionsResponse.html#method.get_address_internal_txs) method for a `GET` operation associated with `GetAddressesAddressHashInternalTransactionsResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashInternalTransactionsResponseGetBuilder<AddressHash> {
    inner: GetAddressesAddressHashInternalTransactionsResponseGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct GetAddressesAddressHashInternalTransactionsResponseGetBuilderContainer {
    param_address_hash: Option<String>,
    param_filter: Option<String>,
}

impl<AddressHash> GetAddressesAddressHashInternalTransactionsResponseGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> GetAddressesAddressHashInternalTransactionsResponseGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn filter(mut self, value: impl Into<String>) -> Self {
        self.inner.param_filter = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetAddressesAddressHashInternalTransactionsResponseGetBuilder<crate::generics::AddressHashExists> {
    type Output = GetAddressesAddressHashInternalTransactionsResponse;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/addresses/{address_hash}/internal-transactions", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .query(&[
            ("filter", self.inner.param_filter.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

