#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTokensAddressHashTokenTransfersResponse<Any> {
    pub items: Vec<crate::token_transfer::TokenTransfer<Any>>,
    pub next_page_params: crate::get_tokens_address_hash_token_transfers_response::GetTokensAddressHashTokenTransfersResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTokensAddressHashTokenTransfersResponseNextPageParams {}

impl<Any: Default> GetTokensAddressHashTokenTransfersResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetTokensAddressHashTokenTransfersResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams, Any> {
        GetTokensAddressHashTokenTransfersResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_token_token_transfers() -> GetTokensAddressHashTokenTransfersResponseGetBuilder<crate::generics::MissingAddressHash> {
        GetTokensAddressHashTokenTransfersResponseGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl<Any> Into<GetTokensAddressHashTokenTransfersResponse<Any>> for GetTokensAddressHashTokenTransfersResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists, Any> {
    fn into(self) -> GetTokensAddressHashTokenTransfersResponse<Any> {
        self.body
    }
}

/// Builder for [`GetTokensAddressHashTokenTransfersResponse`](./struct.GetTokensAddressHashTokenTransfersResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetTokensAddressHashTokenTransfersResponseBuilder<Items, NextPageParams, Any> {
    body: self::GetTokensAddressHashTokenTransfersResponse<Any>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams, Any> GetTokensAddressHashTokenTransfersResponseBuilder<Items, NextPageParams, Any> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::token_transfer::TokenTransferBuilder<crate::generics::FromExists, crate::generics::ToExists, crate::generics::TokenExists, crate::generics::TotalExists, crate::generics::TxHashExists, crate::generics::TypeExists, Any>>) -> GetTokensAddressHashTokenTransfersResponseBuilder<crate::generics::ItemsExists, NextPageParams, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_tokens_address_hash_token_transfers_response::GetTokensAddressHashTokenTransfersResponseNextPageParams) -> GetTokensAddressHashTokenTransfersResponseBuilder<Items, crate::generics::NextPageParamsExists, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetTokensAddressHashTokenTransfersResponse::get_token_token_transfers`](./struct.GetTokensAddressHashTokenTransfersResponse.html#method.get_token_token_transfers) method for a `GET` operation associated with `GetTokensAddressHashTokenTransfersResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetTokensAddressHashTokenTransfersResponseGetBuilder<AddressHash> {
    inner: GetTokensAddressHashTokenTransfersResponseGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct GetTokensAddressHashTokenTransfersResponseGetBuilderContainer {
    param_address_hash: Option<String>,
}

impl<AddressHash> GetTokensAddressHashTokenTransfersResponseGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> GetTokensAddressHashTokenTransfersResponseGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetTokensAddressHashTokenTransfersResponseGetBuilder<crate::generics::AddressHashExists> {
    type Output = GetTokensAddressHashTokenTransfersResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/tokens/{address_hash}/token-transfers", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}

