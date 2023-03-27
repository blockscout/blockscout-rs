#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AddressCounters {
    pub gas_usage_count: String,
    pub token_transfers_count: String,
    pub transactions_count: String,
    pub validations_count: String,
}

impl AddressCounters {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> AddressCountersBuilder<crate::generics::MissingGasUsageCount, crate::generics::MissingTokenTransfersCount, crate::generics::MissingTransactionsCount, crate::generics::MissingValidationsCount> {
        AddressCountersBuilder {
            body: Default::default(),
            _gas_usage_count: core::marker::PhantomData,
            _token_transfers_count: core::marker::PhantomData,
            _transactions_count: core::marker::PhantomData,
            _validations_count: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_address_counters() -> AddressCountersGetBuilder<crate::generics::MissingAddressHash> {
        AddressCountersGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl Into<AddressCounters> for AddressCountersBuilder<crate::generics::GasUsageCountExists, crate::generics::TokenTransfersCountExists, crate::generics::TransactionsCountExists, crate::generics::ValidationsCountExists> {
    fn into(self) -> AddressCounters {
        self.body
    }
}

/// Builder for [`AddressCounters`](./struct.AddressCounters.html) object.
#[derive(Debug, Clone)]
pub struct AddressCountersBuilder<GasUsageCount, TokenTransfersCount, TransactionsCount, ValidationsCount> {
    body: self::AddressCounters,
    _gas_usage_count: core::marker::PhantomData<GasUsageCount>,
    _token_transfers_count: core::marker::PhantomData<TokenTransfersCount>,
    _transactions_count: core::marker::PhantomData<TransactionsCount>,
    _validations_count: core::marker::PhantomData<ValidationsCount>,
}

impl<GasUsageCount, TokenTransfersCount, TransactionsCount, ValidationsCount> AddressCountersBuilder<GasUsageCount, TokenTransfersCount, TransactionsCount, ValidationsCount> {
    #[inline]
    pub fn gas_usage_count(mut self, value: impl Into<String>) -> AddressCountersBuilder<crate::generics::GasUsageCountExists, TokenTransfersCount, TransactionsCount, ValidationsCount> {
        self.body.gas_usage_count = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn token_transfers_count(mut self, value: impl Into<String>) -> AddressCountersBuilder<GasUsageCount, crate::generics::TokenTransfersCountExists, TransactionsCount, ValidationsCount> {
        self.body.token_transfers_count = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn transactions_count(mut self, value: impl Into<String>) -> AddressCountersBuilder<GasUsageCount, TokenTransfersCount, crate::generics::TransactionsCountExists, ValidationsCount> {
        self.body.transactions_count = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn validations_count(mut self, value: impl Into<String>) -> AddressCountersBuilder<GasUsageCount, TokenTransfersCount, TransactionsCount, crate::generics::ValidationsCountExists> {
        self.body.validations_count = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`AddressCounters::get_address_counters`](./struct.AddressCounters.html#method.get_address_counters) method for a `GET` operation associated with `AddressCounters`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct AddressCountersGetBuilder<AddressHash> {
    inner: AddressCountersGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct AddressCountersGetBuilderContainer {
    param_address_hash: Option<String>,
}

impl<AddressHash> AddressCountersGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> AddressCountersGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for AddressCountersGetBuilder<crate::generics::AddressHashExists> {
    type Output = AddressCounters;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/addresses/{address_hash}/counters", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}
