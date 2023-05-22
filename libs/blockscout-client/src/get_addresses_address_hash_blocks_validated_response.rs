#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashBlocksValidatedResponse {
    pub items: Vec<crate::block::Block>,
    pub next_page_params: crate::get_addresses_address_hash_blocks_validated_response::GetAddressesAddressHashBlocksValidatedResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashBlocksValidatedResponseNextPageParams {}

impl GetAddressesAddressHashBlocksValidatedResponse {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetAddressesAddressHashBlocksValidatedResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams> {
        GetAddressesAddressHashBlocksValidatedResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_address_blocks_validated() -> GetAddressesAddressHashBlocksValidatedResponseGetBuilder<crate::generics::MissingAddressHash> {
        GetAddressesAddressHashBlocksValidatedResponseGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl Into<GetAddressesAddressHashBlocksValidatedResponse> for GetAddressesAddressHashBlocksValidatedResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists> {
    fn into(self) -> GetAddressesAddressHashBlocksValidatedResponse {
        self.body
    }
}

/// Builder for [`GetAddressesAddressHashBlocksValidatedResponse`](./struct.GetAddressesAddressHashBlocksValidatedResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashBlocksValidatedResponseBuilder<Items, NextPageParams> {
    body: self::GetAddressesAddressHashBlocksValidatedResponse,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams> GetAddressesAddressHashBlocksValidatedResponseBuilder<Items, NextPageParams> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::block::BlockBuilder<crate::generics::DifficultyExists, crate::generics::GasLimitExists, crate::generics::GasUsedExists, crate::generics::HashExists, crate::generics::HeightExists, crate::generics::MinerExists, crate::generics::NonceExists, crate::generics::ParentHashExists, crate::generics::SizeExists, crate::generics::TimestampExists, crate::generics::TotalDifficultyExists, crate::generics::TxCountExists>>) -> GetAddressesAddressHashBlocksValidatedResponseBuilder<crate::generics::ItemsExists, NextPageParams> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_addresses_address_hash_blocks_validated_response::GetAddressesAddressHashBlocksValidatedResponseNextPageParams) -> GetAddressesAddressHashBlocksValidatedResponseBuilder<Items, crate::generics::NextPageParamsExists> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetAddressesAddressHashBlocksValidatedResponse::get_address_blocks_validated`](./struct.GetAddressesAddressHashBlocksValidatedResponse.html#method.get_address_blocks_validated) method for a `GET` operation associated with `GetAddressesAddressHashBlocksValidatedResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashBlocksValidatedResponseGetBuilder<AddressHash> {
    inner: GetAddressesAddressHashBlocksValidatedResponseGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct GetAddressesAddressHashBlocksValidatedResponseGetBuilderContainer {
    param_address_hash: Option<String>,
}

impl<AddressHash> GetAddressesAddressHashBlocksValidatedResponseGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> GetAddressesAddressHashBlocksValidatedResponseGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetAddressesAddressHashBlocksValidatedResponseGetBuilder<crate::generics::AddressHashExists> {
    type Output = GetAddressesAddressHashBlocksValidatedResponse;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/addresses/{address_hash}/blocks-validated", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}

