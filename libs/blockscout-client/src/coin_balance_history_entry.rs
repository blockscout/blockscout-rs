#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CoinBalanceHistoryEntry {
    pub block_number: i64,
    pub block_timestamp: String,
    pub delta: String,
    pub transaction_hash: Option<String>,
    pub value: String,
}

impl CoinBalanceHistoryEntry {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> CoinBalanceHistoryEntryBuilder<crate::generics::MissingBlockNumber, crate::generics::MissingBlockTimestamp, crate::generics::MissingDelta, crate::generics::MissingValue> {
        CoinBalanceHistoryEntryBuilder {
            body: Default::default(),
            _block_number: core::marker::PhantomData,
            _block_timestamp: core::marker::PhantomData,
            _delta: core::marker::PhantomData,
            _value: core::marker::PhantomData,
        }
    }
}

impl Into<CoinBalanceHistoryEntry> for CoinBalanceHistoryEntryBuilder<crate::generics::BlockNumberExists, crate::generics::BlockTimestampExists, crate::generics::DeltaExists, crate::generics::ValueExists> {
    fn into(self) -> CoinBalanceHistoryEntry {
        self.body
    }
}

/// Builder for [`CoinBalanceHistoryEntry`](./struct.CoinBalanceHistoryEntry.html) object.
#[derive(Debug, Clone)]
pub struct CoinBalanceHistoryEntryBuilder<BlockNumber, BlockTimestamp, Delta, Value> {
    body: self::CoinBalanceHistoryEntry,
    _block_number: core::marker::PhantomData<BlockNumber>,
    _block_timestamp: core::marker::PhantomData<BlockTimestamp>,
    _delta: core::marker::PhantomData<Delta>,
    _value: core::marker::PhantomData<Value>,
}

impl<BlockNumber, BlockTimestamp, Delta, Value> CoinBalanceHistoryEntryBuilder<BlockNumber, BlockTimestamp, Delta, Value> {
    #[inline]
    pub fn block_number(mut self, value: impl Into<i64>) -> CoinBalanceHistoryEntryBuilder<crate::generics::BlockNumberExists, BlockTimestamp, Delta, Value> {
        self.body.block_number = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn block_timestamp(mut self, value: impl Into<String>) -> CoinBalanceHistoryEntryBuilder<BlockNumber, crate::generics::BlockTimestampExists, Delta, Value> {
        self.body.block_timestamp = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn delta(mut self, value: impl Into<String>) -> CoinBalanceHistoryEntryBuilder<BlockNumber, BlockTimestamp, crate::generics::DeltaExists, Value> {
        self.body.delta = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn transaction_hash(mut self, value: impl Into<String>) -> Self {
        self.body.transaction_hash = Some(value.into());
        self
    }

    #[inline]
    pub fn value(mut self, value: impl Into<String>) -> CoinBalanceHistoryEntryBuilder<BlockNumber, BlockTimestamp, Delta, crate::generics::ValueExists> {
        self.body.value = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
