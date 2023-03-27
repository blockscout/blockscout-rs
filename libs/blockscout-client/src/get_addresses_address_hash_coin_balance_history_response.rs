#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashCoinBalanceHistoryResponse {
    pub items: Vec<crate::coin_balance_history_entry::CoinBalanceHistoryEntry>,
    pub next_page_params: crate::get_addresses_address_hash_coin_balance_history_response::GetAddressesAddressHashCoinBalanceHistoryResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetAddressesAddressHashCoinBalanceHistoryResponseNextPageParams {}

impl GetAddressesAddressHashCoinBalanceHistoryResponse {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetAddressesAddressHashCoinBalanceHistoryResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams> {
        GetAddressesAddressHashCoinBalanceHistoryResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_address_coin_balance_history() -> GetAddressesAddressHashCoinBalanceHistoryResponseGetBuilder<crate::generics::MissingAddressHash> {
        GetAddressesAddressHashCoinBalanceHistoryResponseGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl Into<GetAddressesAddressHashCoinBalanceHistoryResponse> for GetAddressesAddressHashCoinBalanceHistoryResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists> {
    fn into(self) -> GetAddressesAddressHashCoinBalanceHistoryResponse {
        self.body
    }
}

/// Builder for [`GetAddressesAddressHashCoinBalanceHistoryResponse`](./struct.GetAddressesAddressHashCoinBalanceHistoryResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashCoinBalanceHistoryResponseBuilder<Items, NextPageParams> {
    body: self::GetAddressesAddressHashCoinBalanceHistoryResponse,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams> GetAddressesAddressHashCoinBalanceHistoryResponseBuilder<Items, NextPageParams> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::coin_balance_history_entry::CoinBalanceHistoryEntryBuilder<crate::generics::BlockNumberExists, crate::generics::BlockTimestampExists, crate::generics::DeltaExists, crate::generics::ValueExists>>) -> GetAddressesAddressHashCoinBalanceHistoryResponseBuilder<crate::generics::ItemsExists, NextPageParams> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_addresses_address_hash_coin_balance_history_response::GetAddressesAddressHashCoinBalanceHistoryResponseNextPageParams) -> GetAddressesAddressHashCoinBalanceHistoryResponseBuilder<Items, crate::generics::NextPageParamsExists> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetAddressesAddressHashCoinBalanceHistoryResponse::get_address_coin_balance_history`](./struct.GetAddressesAddressHashCoinBalanceHistoryResponse.html#method.get_address_coin_balance_history) method for a `GET` operation associated with `GetAddressesAddressHashCoinBalanceHistoryResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetAddressesAddressHashCoinBalanceHistoryResponseGetBuilder<AddressHash> {
    inner: GetAddressesAddressHashCoinBalanceHistoryResponseGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct GetAddressesAddressHashCoinBalanceHistoryResponseGetBuilderContainer {
    param_address_hash: Option<String>,
}

impl<AddressHash> GetAddressesAddressHashCoinBalanceHistoryResponseGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> GetAddressesAddressHashCoinBalanceHistoryResponseGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetAddressesAddressHashCoinBalanceHistoryResponseGetBuilder<crate::generics::AddressHashExists> {
    type Output = GetAddressesAddressHashCoinBalanceHistoryResponse;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/addresses/{address_hash}/coin-balance-history", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}

