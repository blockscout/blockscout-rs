#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTokensAddressHashInstancesResponse<Any> {
    pub items: Vec<Any>,
    pub next_page_params: crate::get_tokens_address_hash_instances_response::GetTokensAddressHashInstancesResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTokensAddressHashInstancesResponseNextPageParams {}

impl<Any: Default> GetTokensAddressHashInstancesResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetTokensAddressHashInstancesResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams, Any> {
        GetTokensAddressHashInstancesResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_nft_instances() -> GetTokensAddressHashInstancesResponseGetBuilder<crate::generics::MissingAddressHash> {
        GetTokensAddressHashInstancesResponseGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl<Any> Into<GetTokensAddressHashInstancesResponse<Any>> for GetTokensAddressHashInstancesResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists, Any> {
    fn into(self) -> GetTokensAddressHashInstancesResponse<Any> {
        self.body
    }
}

/// Builder for [`GetTokensAddressHashInstancesResponse`](./struct.GetTokensAddressHashInstancesResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetTokensAddressHashInstancesResponseBuilder<Items, NextPageParams, Any> {
    body: self::GetTokensAddressHashInstancesResponse<Any>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams, Any> GetTokensAddressHashInstancesResponseBuilder<Items, NextPageParams, Any> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = impl Into<Any>>) -> GetTokensAddressHashInstancesResponseBuilder<crate::generics::ItemsExists, NextPageParams, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_tokens_address_hash_instances_response::GetTokensAddressHashInstancesResponseNextPageParams) -> GetTokensAddressHashInstancesResponseBuilder<Items, crate::generics::NextPageParamsExists, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetTokensAddressHashInstancesResponse::get_nft_instances`](./struct.GetTokensAddressHashInstancesResponse.html#method.get_nft_instances) method for a `GET` operation associated with `GetTokensAddressHashInstancesResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetTokensAddressHashInstancesResponseGetBuilder<AddressHash> {
    inner: GetTokensAddressHashInstancesResponseGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct GetTokensAddressHashInstancesResponseGetBuilderContainer {
    param_address_hash: Option<String>,
}

impl<AddressHash> GetTokensAddressHashInstancesResponseGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> GetTokensAddressHashInstancesResponseGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetTokensAddressHashInstancesResponseGetBuilder<crate::generics::AddressHashExists> {
    type Output = GetTokensAddressHashInstancesResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/tokens/{address_hash}/instances", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}

