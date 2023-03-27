#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CoinBalanceHistoryByDaysEntry {
    pub date: String,
    pub value: f64,
}

impl CoinBalanceHistoryByDaysEntry {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> CoinBalanceHistoryByDaysEntryBuilder<crate::generics::MissingDate, crate::generics::MissingValue> {
        CoinBalanceHistoryByDaysEntryBuilder {
            body: Default::default(),
            _date: core::marker::PhantomData,
            _value: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_address_coin_balance_history_by_day() -> CoinBalanceHistoryByDaysEntryGetBuilder<crate::generics::MissingAddressHash> {
        CoinBalanceHistoryByDaysEntryGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl Into<CoinBalanceHistoryByDaysEntry> for CoinBalanceHistoryByDaysEntryBuilder<crate::generics::DateExists, crate::generics::ValueExists> {
    fn into(self) -> CoinBalanceHistoryByDaysEntry {
        self.body
    }
}

/// Builder for [`CoinBalanceHistoryByDaysEntry`](./struct.CoinBalanceHistoryByDaysEntry.html) object.
#[derive(Debug, Clone)]
pub struct CoinBalanceHistoryByDaysEntryBuilder<Date, Value> {
    body: self::CoinBalanceHistoryByDaysEntry,
    _date: core::marker::PhantomData<Date>,
    _value: core::marker::PhantomData<Value>,
}

impl<Date, Value> CoinBalanceHistoryByDaysEntryBuilder<Date, Value> {
    #[inline]
    pub fn date(mut self, value: impl Into<String>) -> CoinBalanceHistoryByDaysEntryBuilder<crate::generics::DateExists, Value> {
        self.body.date = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn value(mut self, value: impl Into<f64>) -> CoinBalanceHistoryByDaysEntryBuilder<Date, crate::generics::ValueExists> {
        self.body.value = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`CoinBalanceHistoryByDaysEntry::get_address_coin_balance_history_by_day`](./struct.CoinBalanceHistoryByDaysEntry.html#method.get_address_coin_balance_history_by_day) method for a `GET` operation associated with `CoinBalanceHistoryByDaysEntry`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct CoinBalanceHistoryByDaysEntryGetBuilder<AddressHash> {
    inner: CoinBalanceHistoryByDaysEntryGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct CoinBalanceHistoryByDaysEntryGetBuilderContainer {
    param_address_hash: Option<String>,
}

impl<AddressHash> CoinBalanceHistoryByDaysEntryGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> CoinBalanceHistoryByDaysEntryGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for CoinBalanceHistoryByDaysEntryGetBuilder<crate::generics::AddressHashExists> {
    type Output = Vec<CoinBalanceHistoryByDaysEntry>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/addresses/{address_hash}/coin-balance-history-by-day", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}
