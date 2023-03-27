#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesResponse<Any> {
    pub exchange_rate: String,
    pub items: Vec<Any>,
    pub next_page_params: crate::get_addresses_response::GetAddressesResponseNextPageParams,
    pub total_supply: String,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesResponseNextPageParams {}

impl<Any: Default> GetAddressesResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetAddressesResponseBuilder<crate::generics::MissingExchangeRate, crate::generics::MissingItems, crate::generics::MissingNextPageParams, crate::generics::MissingTotalSupply, Any> {
        GetAddressesResponseBuilder {
            body: Default::default(),
            _exchange_rate: core::marker::PhantomData,
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
            _total_supply: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_addresses() -> GetAddressesResponseGetBuilder {
        GetAddressesResponseGetBuilder
    }
}

impl<Any> Into<GetAddressesResponse<Any>> for GetAddressesResponseBuilder<crate::generics::ExchangeRateExists, crate::generics::ItemsExists, crate::generics::NextPageParamsExists, crate::generics::TotalSupplyExists, Any> {
    fn into(self) -> GetAddressesResponse<Any> {
        self.body
    }
}

/// Builder for [`GetAddressesResponse`](./struct.GetAddressesResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetAddressesResponseBuilder<ExchangeRate, Items, NextPageParams, TotalSupply, Any> {
    body: self::GetAddressesResponse<Any>,
    _exchange_rate: core::marker::PhantomData<ExchangeRate>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
    _total_supply: core::marker::PhantomData<TotalSupply>,
}

impl<ExchangeRate, Items, NextPageParams, TotalSupply, Any> GetAddressesResponseBuilder<ExchangeRate, Items, NextPageParams, TotalSupply, Any> {
    #[inline]
    pub fn exchange_rate(mut self, value: impl Into<String>) -> GetAddressesResponseBuilder<crate::generics::ExchangeRateExists, Items, NextPageParams, TotalSupply, Any> {
        self.body.exchange_rate = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = impl Into<Any>>) -> GetAddressesResponseBuilder<ExchangeRate, crate::generics::ItemsExists, NextPageParams, TotalSupply, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_addresses_response::GetAddressesResponseNextPageParams) -> GetAddressesResponseBuilder<ExchangeRate, Items, crate::generics::NextPageParamsExists, TotalSupply, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn total_supply(mut self, value: impl Into<String>) -> GetAddressesResponseBuilder<ExchangeRate, Items, NextPageParams, crate::generics::TotalSupplyExists, Any> {
        self.body.total_supply = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetAddressesResponse::get_addresses`](./struct.GetAddressesResponse.html#method.get_addresses) method for a `GET` operation associated with `GetAddressesResponse`.
#[derive(Debug, Clone)]
pub struct GetAddressesResponseGetBuilder;


impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetAddressesResponseGetBuilder {
    type Output = GetAddressesResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/addresses".into()
    }
}

