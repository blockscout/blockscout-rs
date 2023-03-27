#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct InternalTransaction {
    pub block: i64,
    pub created_contract: crate::address_param::AddressParam,
    pub error: Option<String>,
    pub from: crate::address_param::AddressParam,
    pub index: i64,
    pub success: bool,
    pub timestamp: String,
    pub to: crate::address_param::AddressParam,
    pub transaction_hash: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub value: i64,
}

impl InternalTransaction {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> InternalTransactionBuilder<crate::generics::MissingBlock, crate::generics::MissingCreatedContract, crate::generics::MissingFrom, crate::generics::MissingIndex, crate::generics::MissingSuccess, crate::generics::MissingTimestamp, crate::generics::MissingTo, crate::generics::MissingTransactionHash, crate::generics::MissingType, crate::generics::MissingValue> {
        InternalTransactionBuilder {
            body: Default::default(),
            _block: core::marker::PhantomData,
            _created_contract: core::marker::PhantomData,
            _from: core::marker::PhantomData,
            _index: core::marker::PhantomData,
            _success: core::marker::PhantomData,
            _timestamp: core::marker::PhantomData,
            _to: core::marker::PhantomData,
            _transaction_hash: core::marker::PhantomData,
            _type: core::marker::PhantomData,
            _value: core::marker::PhantomData,
        }
    }
}

impl Into<InternalTransaction> for InternalTransactionBuilder<crate::generics::BlockExists, crate::generics::CreatedContractExists, crate::generics::FromExists, crate::generics::IndexExists, crate::generics::SuccessExists, crate::generics::TimestampExists, crate::generics::ToExists, crate::generics::TransactionHashExists, crate::generics::TypeExists, crate::generics::ValueExists> {
    fn into(self) -> InternalTransaction {
        self.body
    }
}

/// Builder for [`InternalTransaction`](./struct.InternalTransaction.html) object.
#[derive(Debug, Clone)]
pub struct InternalTransactionBuilder<Block, CreatedContract, From, Index, Success, Timestamp, To, TransactionHash, Type, Value> {
    body: self::InternalTransaction,
    _block: core::marker::PhantomData<Block>,
    _created_contract: core::marker::PhantomData<CreatedContract>,
    _from: core::marker::PhantomData<From>,
    _index: core::marker::PhantomData<Index>,
    _success: core::marker::PhantomData<Success>,
    _timestamp: core::marker::PhantomData<Timestamp>,
    _to: core::marker::PhantomData<To>,
    _transaction_hash: core::marker::PhantomData<TransactionHash>,
    _type: core::marker::PhantomData<Type>,
    _value: core::marker::PhantomData<Value>,
}

impl<Block, CreatedContract, From, Index, Success, Timestamp, To, TransactionHash, Type, Value> InternalTransactionBuilder<Block, CreatedContract, From, Index, Success, Timestamp, To, TransactionHash, Type, Value> {
    #[inline]
    pub fn block(mut self, value: impl Into<i64>) -> InternalTransactionBuilder<crate::generics::BlockExists, CreatedContract, From, Index, Success, Timestamp, To, TransactionHash, Type, Value> {
        self.body.block = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn created_contract(mut self, value: crate::address_param::AddressParamBuilder<crate::generics::HashExists>) -> InternalTransactionBuilder<Block, crate::generics::CreatedContractExists, From, Index, Success, Timestamp, To, TransactionHash, Type, Value> {
        self.body.created_contract = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn error(mut self, value: impl Into<String>) -> Self {
        self.body.error = Some(value.into());
        self
    }

    #[inline]
    pub fn from(mut self, value: crate::address_param::AddressParamBuilder<crate::generics::HashExists>) -> InternalTransactionBuilder<Block, CreatedContract, crate::generics::FromExists, Index, Success, Timestamp, To, TransactionHash, Type, Value> {
        self.body.from = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn index(mut self, value: impl Into<i64>) -> InternalTransactionBuilder<Block, CreatedContract, From, crate::generics::IndexExists, Success, Timestamp, To, TransactionHash, Type, Value> {
        self.body.index = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn success(mut self, value: impl Into<bool>) -> InternalTransactionBuilder<Block, CreatedContract, From, Index, crate::generics::SuccessExists, Timestamp, To, TransactionHash, Type, Value> {
        self.body.success = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn timestamp(mut self, value: impl Into<String>) -> InternalTransactionBuilder<Block, CreatedContract, From, Index, Success, crate::generics::TimestampExists, To, TransactionHash, Type, Value> {
        self.body.timestamp = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn to(mut self, value: crate::address_param::AddressParamBuilder<crate::generics::HashExists>) -> InternalTransactionBuilder<Block, CreatedContract, From, Index, Success, Timestamp, crate::generics::ToExists, TransactionHash, Type, Value> {
        self.body.to = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn transaction_hash(mut self, value: impl Into<String>) -> InternalTransactionBuilder<Block, CreatedContract, From, Index, Success, Timestamp, To, crate::generics::TransactionHashExists, Type, Value> {
        self.body.transaction_hash = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> InternalTransactionBuilder<Block, CreatedContract, From, Index, Success, Timestamp, To, TransactionHash, crate::generics::TypeExists, Value> {
        self.body.type_ = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn value(mut self, value: impl Into<i64>) -> InternalTransactionBuilder<Block, CreatedContract, From, Index, Success, Timestamp, To, TransactionHash, Type, crate::generics::ValueExists> {
        self.body.value = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
