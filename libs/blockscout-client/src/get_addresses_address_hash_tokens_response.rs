#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashTokensResponse {
    pub items: Vec<crate::token_balance::TokenBalance>,
    pub next_page_params: crate::get_addresses_address_hash_tokens_response::GetAddressesAddressHashTokensResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashTokensResponseNextPageParams {}

impl GetAddressesAddressHashTokensResponse {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetAddressesAddressHashTokensResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams> {
        GetAddressesAddressHashTokensResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_address_tokens() -> GetAddressesAddressHashTokensResponseGetBuilder<crate::generics::MissingAddressHash> {
        GetAddressesAddressHashTokensResponseGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl Into<GetAddressesAddressHashTokensResponse> for GetAddressesAddressHashTokensResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists> {
    fn into(self) -> GetAddressesAddressHashTokensResponse {
        self.body
    }
}

/// Builder for [`GetAddressesAddressHashTokensResponse`](./struct.GetAddressesAddressHashTokensResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashTokensResponseBuilder<Items, NextPageParams> {
    body: self::GetAddressesAddressHashTokensResponse,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams> GetAddressesAddressHashTokensResponseBuilder<Items, NextPageParams> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::token_balance::TokenBalanceBuilder<crate::generics::TokenExists, crate::generics::TokenIdExists, crate::generics::ValueExists>>) -> GetAddressesAddressHashTokensResponseBuilder<crate::generics::ItemsExists, NextPageParams> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_addresses_address_hash_tokens_response::GetAddressesAddressHashTokensResponseNextPageParams) -> GetAddressesAddressHashTokensResponseBuilder<Items, crate::generics::NextPageParamsExists> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetAddressesAddressHashTokensResponse::get_address_tokens`](./struct.GetAddressesAddressHashTokensResponse.html#method.get_address_tokens) method for a `GET` operation associated with `GetAddressesAddressHashTokensResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashTokensResponseGetBuilder<AddressHash> {
    inner: GetAddressesAddressHashTokensResponseGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct GetAddressesAddressHashTokensResponseGetBuilderContainer {
    param_address_hash: Option<String>,
    param_type: Option<String>,
}

impl<AddressHash> GetAddressesAddressHashTokensResponseGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> GetAddressesAddressHashTokensResponseGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> Self {
        self.inner.param_type = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetAddressesAddressHashTokensResponseGetBuilder<crate::generics::AddressHashExists> {
    type Output = GetAddressesAddressHashTokensResponse;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/addresses/{address_hash}/tokens", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .query(&[
            ("type", self.inner.param_type.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

