#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashLogsResponse {
    pub items: Vec<crate::log::Log>,
    pub next_page_params: crate::get_addresses_address_hash_logs_response::GetAddressesAddressHashLogsResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashLogsResponseNextPageParams {}

impl GetAddressesAddressHashLogsResponse {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetAddressesAddressHashLogsResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams> {
        GetAddressesAddressHashLogsResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_address_logs() -> GetAddressesAddressHashLogsResponseGetBuilder<crate::generics::MissingAddressHash> {
        GetAddressesAddressHashLogsResponseGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl Into<GetAddressesAddressHashLogsResponse> for GetAddressesAddressHashLogsResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists> {
    fn into(self) -> GetAddressesAddressHashLogsResponse {
        self.body
    }
}

/// Builder for [`GetAddressesAddressHashLogsResponse`](./struct.GetAddressesAddressHashLogsResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashLogsResponseBuilder<Items, NextPageParams> {
    body: self::GetAddressesAddressHashLogsResponse,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams> GetAddressesAddressHashLogsResponseBuilder<Items, NextPageParams> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::log::LogBuilder<crate::generics::AddressExists, crate::generics::DataExists, crate::generics::IndexExists, crate::generics::TopicsExists, crate::generics::TxHashExists>>) -> GetAddressesAddressHashLogsResponseBuilder<crate::generics::ItemsExists, NextPageParams> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_addresses_address_hash_logs_response::GetAddressesAddressHashLogsResponseNextPageParams) -> GetAddressesAddressHashLogsResponseBuilder<Items, crate::generics::NextPageParamsExists> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetAddressesAddressHashLogsResponse::get_address_logs`](./struct.GetAddressesAddressHashLogsResponse.html#method.get_address_logs) method for a `GET` operation associated with `GetAddressesAddressHashLogsResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashLogsResponseGetBuilder<AddressHash> {
    inner: GetAddressesAddressHashLogsResponseGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct GetAddressesAddressHashLogsResponseGetBuilderContainer {
    param_address_hash: Option<String>,
}

impl<AddressHash> GetAddressesAddressHashLogsResponseGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> GetAddressesAddressHashLogsResponseGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetAddressesAddressHashLogsResponseGetBuilder<crate::generics::AddressHashExists> {
    type Output = GetAddressesAddressHashLogsResponse;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/addresses/{address_hash}/logs", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}

