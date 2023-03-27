#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Log {
    pub address: crate::address_param::AddressParam,
    pub data: String,
    pub decoded: Option<crate::decoded_input_log::DecodedInputLog>,
    pub index: i64,
    pub topics: Vec<String>,
    pub tx_hash: String,
}

impl Log {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> LogBuilder<crate::generics::MissingAddress, crate::generics::MissingData, crate::generics::MissingIndex, crate::generics::MissingTopics, crate::generics::MissingTxHash> {
        LogBuilder {
            body: Default::default(),
            _address: core::marker::PhantomData,
            _data: core::marker::PhantomData,
            _index: core::marker::PhantomData,
            _topics: core::marker::PhantomData,
            _tx_hash: core::marker::PhantomData,
        }
    }
}

impl Into<Log> for LogBuilder<crate::generics::AddressExists, crate::generics::DataExists, crate::generics::IndexExists, crate::generics::TopicsExists, crate::generics::TxHashExists> {
    fn into(self) -> Log {
        self.body
    }
}

/// Builder for [`Log`](./struct.Log.html) object.
#[derive(Debug, Clone)]
pub struct LogBuilder<Address, Data, Index, Topics, TxHash> {
    body: self::Log,
    _address: core::marker::PhantomData<Address>,
    _data: core::marker::PhantomData<Data>,
    _index: core::marker::PhantomData<Index>,
    _topics: core::marker::PhantomData<Topics>,
    _tx_hash: core::marker::PhantomData<TxHash>,
}

impl<Address, Data, Index, Topics, TxHash> LogBuilder<Address, Data, Index, Topics, TxHash> {
    #[inline]
    pub fn address(mut self, value: crate::address_param::AddressParamBuilder<crate::generics::HashExists>) -> LogBuilder<crate::generics::AddressExists, Data, Index, Topics, TxHash> {
        self.body.address = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn data(mut self, value: impl Into<String>) -> LogBuilder<Address, crate::generics::DataExists, Index, Topics, TxHash> {
        self.body.data = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn decoded(mut self, value: crate::decoded_input_log::DecodedInputLogBuilder<crate::generics::MethodCallExists, crate::generics::MethodIdExists, crate::generics::ParametersExists>) -> Self {
        self.body.decoded = Some(value.into());
        self
    }

    #[inline]
    pub fn index(mut self, value: impl Into<i64>) -> LogBuilder<Address, Data, crate::generics::IndexExists, Topics, TxHash> {
        self.body.index = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn topics(mut self, value: impl Iterator<Item = impl Into<String>>) -> LogBuilder<Address, Data, Index, crate::generics::TopicsExists, TxHash> {
        self.body.topics = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn tx_hash(mut self, value: impl Into<String>) -> LogBuilder<Address, Data, Index, Topics, crate::generics::TxHashExists> {
        self.body.tx_hash = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
