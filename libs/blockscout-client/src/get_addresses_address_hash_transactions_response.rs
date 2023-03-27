#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashTransactionsResponse<Any> {
    pub items: Vec<Any>,
    pub next_page_params: crate::get_addresses_address_hash_transactions_response::GetAddressesAddressHashTransactionsResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashTransactionsResponseNextPageParams {}

impl<Any: Default> GetAddressesAddressHashTransactionsResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetAddressesAddressHashTransactionsResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams, Any> {
        GetAddressesAddressHashTransactionsResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_address_txs() -> GetAddressesAddressHashTransactionsResponseGetBuilder<crate::generics::MissingAddressHash> {
        GetAddressesAddressHashTransactionsResponseGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl<Any> Into<GetAddressesAddressHashTransactionsResponse<Any>> for GetAddressesAddressHashTransactionsResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists, Any> {
    fn into(self) -> GetAddressesAddressHashTransactionsResponse<Any> {
        self.body
    }
}

/// Builder for [`GetAddressesAddressHashTransactionsResponse`](./struct.GetAddressesAddressHashTransactionsResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashTransactionsResponseBuilder<Items, NextPageParams, Any> {
    body: self::GetAddressesAddressHashTransactionsResponse<Any>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams, Any> GetAddressesAddressHashTransactionsResponseBuilder<Items, NextPageParams, Any> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = impl Into<Any>>) -> GetAddressesAddressHashTransactionsResponseBuilder<crate::generics::ItemsExists, NextPageParams, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_addresses_address_hash_transactions_response::GetAddressesAddressHashTransactionsResponseNextPageParams) -> GetAddressesAddressHashTransactionsResponseBuilder<Items, crate::generics::NextPageParamsExists, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetAddressesAddressHashTransactionsResponse::get_address_txs`](./struct.GetAddressesAddressHashTransactionsResponse.html#method.get_address_txs) method for a `GET` operation associated with `GetAddressesAddressHashTransactionsResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashTransactionsResponseGetBuilder<AddressHash> {
    inner: GetAddressesAddressHashTransactionsResponseGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct GetAddressesAddressHashTransactionsResponseGetBuilderContainer {
    param_address_hash: Option<String>,
    param_filter: Option<String>,
}

impl<AddressHash> GetAddressesAddressHashTransactionsResponseGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> GetAddressesAddressHashTransactionsResponseGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn filter(mut self, value: impl Into<String>) -> Self {
        self.inner.param_filter = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetAddressesAddressHashTransactionsResponseGetBuilder<crate::generics::AddressHashExists> {
    type Output = GetAddressesAddressHashTransactionsResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/addresses/{address_hash}/transactions", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .header(http::header::ACCEPT.as_str(), "application/yaml")
        .query(&[
            ("filter", self.inner.param_filter.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

