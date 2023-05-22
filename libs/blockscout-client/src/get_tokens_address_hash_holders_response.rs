#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTokensAddressHashHoldersResponse<Any> {
    pub items: Vec<Any>,
    pub next_page_params: crate::get_tokens_address_hash_holders_response::GetTokensAddressHashHoldersResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTokensAddressHashHoldersResponseNextPageParams {}

impl<Any: Default> GetTokensAddressHashHoldersResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetTokensAddressHashHoldersResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams, Any> {
        GetTokensAddressHashHoldersResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_token_holders() -> GetTokensAddressHashHoldersResponseGetBuilder<crate::generics::MissingAddressHash> {
        GetTokensAddressHashHoldersResponseGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl<Any> Into<GetTokensAddressHashHoldersResponse<Any>> for GetTokensAddressHashHoldersResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists, Any> {
    fn into(self) -> GetTokensAddressHashHoldersResponse<Any> {
        self.body
    }
}

/// Builder for [`GetTokensAddressHashHoldersResponse`](./struct.GetTokensAddressHashHoldersResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetTokensAddressHashHoldersResponseBuilder<Items, NextPageParams, Any> {
    body: self::GetTokensAddressHashHoldersResponse<Any>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams, Any> GetTokensAddressHashHoldersResponseBuilder<Items, NextPageParams, Any> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = impl Into<Any>>) -> GetTokensAddressHashHoldersResponseBuilder<crate::generics::ItemsExists, NextPageParams, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_tokens_address_hash_holders_response::GetTokensAddressHashHoldersResponseNextPageParams) -> GetTokensAddressHashHoldersResponseBuilder<Items, crate::generics::NextPageParamsExists, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetTokensAddressHashHoldersResponse::get_token_holders`](./struct.GetTokensAddressHashHoldersResponse.html#method.get_token_holders) method for a `GET` operation associated with `GetTokensAddressHashHoldersResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetTokensAddressHashHoldersResponseGetBuilder<AddressHash> {
    inner: GetTokensAddressHashHoldersResponseGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct GetTokensAddressHashHoldersResponseGetBuilderContainer {
    param_address_hash: Option<String>,
}

impl<AddressHash> GetTokensAddressHashHoldersResponseGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> GetTokensAddressHashHoldersResponseGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetTokensAddressHashHoldersResponseGetBuilder<crate::generics::AddressHashExists> {
    type Output = GetTokensAddressHashHoldersResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/tokens/{address_hash}/holders", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}

