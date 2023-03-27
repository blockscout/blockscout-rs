#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashTokenTransfersResponse<Any> {
    pub items: Vec<crate::token_transfer::TokenTransfer<Any>>,
    pub next_page_params: crate::get_addresses_address_hash_token_transfers_response::GetAddressesAddressHashTokenTransfersResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashTokenTransfersResponseNextPageParams {}

impl<Any: Default> GetAddressesAddressHashTokenTransfersResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetAddressesAddressHashTokenTransfersResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams, Any> {
        GetAddressesAddressHashTokenTransfersResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_address_token_transfers() -> GetAddressesAddressHashTokenTransfersResponseGetBuilder<crate::generics::MissingAddressHash> {
        GetAddressesAddressHashTokenTransfersResponseGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl<Any> Into<GetAddressesAddressHashTokenTransfersResponse<Any>> for GetAddressesAddressHashTokenTransfersResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists, Any> {
    fn into(self) -> GetAddressesAddressHashTokenTransfersResponse<Any> {
        self.body
    }
}

/// Builder for [`GetAddressesAddressHashTokenTransfersResponse`](./struct.GetAddressesAddressHashTokenTransfersResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashTokenTransfersResponseBuilder<Items, NextPageParams, Any> {
    body: self::GetAddressesAddressHashTokenTransfersResponse<Any>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams, Any> GetAddressesAddressHashTokenTransfersResponseBuilder<Items, NextPageParams, Any> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::token_transfer::TokenTransferBuilder<crate::generics::FromExists, crate::generics::ToExists, crate::generics::TokenExists, crate::generics::TotalExists, crate::generics::TxHashExists, crate::generics::TypeExists, Any>>) -> GetAddressesAddressHashTokenTransfersResponseBuilder<crate::generics::ItemsExists, NextPageParams, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_addresses_address_hash_token_transfers_response::GetAddressesAddressHashTokenTransfersResponseNextPageParams) -> GetAddressesAddressHashTokenTransfersResponseBuilder<Items, crate::generics::NextPageParamsExists, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetAddressesAddressHashTokenTransfersResponse::get_address_token_transfers`](./struct.GetAddressesAddressHashTokenTransfersResponse.html#method.get_address_token_transfers) method for a `GET` operation associated with `GetAddressesAddressHashTokenTransfersResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashTokenTransfersResponseGetBuilder<AddressHash> {
    inner: GetAddressesAddressHashTokenTransfersResponseGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct GetAddressesAddressHashTokenTransfersResponseGetBuilderContainer {
    param_address_hash: Option<String>,
    param_type: Option<String>,
    param_filter: Option<String>,
    param_token: Option<String>,
}

impl<AddressHash> GetAddressesAddressHashTokenTransfersResponseGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> GetAddressesAddressHashTokenTransfersResponseGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> Self {
        self.inner.param_type = Some(value.into());
        self
    }

    #[inline]
    pub fn filter(mut self, value: impl Into<String>) -> Self {
        self.inner.param_filter = Some(value.into());
        self
    }

    #[inline]
    pub fn token(mut self, value: impl Into<String>) -> Self {
        self.inner.param_token = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetAddressesAddressHashTokenTransfersResponseGetBuilder<crate::generics::AddressHashExists> {
    type Output = GetAddressesAddressHashTokenTransfersResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/addresses/{address_hash}/token-transfers", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .header(http::header::ACCEPT.as_str(), "application/yaml")
        .query(&[
            ("type", self.inner.param_type.as_ref().map(std::string::ToString::to_string)),
            ("filter", self.inner.param_filter.as_ref().map(std::string::ToString::to_string)),
            ("token", self.inner.param_token.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

