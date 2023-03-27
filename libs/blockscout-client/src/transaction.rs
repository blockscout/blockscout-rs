#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Transaction<Any> {
    pub base_fee_per_gas: Option<i64>,
    pub block: Option<i32>,
    pub confirmation_duration: crate::transaction::TransactionConfirmationDuration,
    pub confirmations: i64,
    pub created_contract: crate::address_param::AddressParam,
    pub decoded_input: Option<crate::decoded_input::DecodedInput>,
    pub exchange_rate: Option<f64>,
    pub fee: crate::fee::Fee,
    pub from: crate::address_param::AddressParam,
    pub gas_limit: i64,
    pub gas_price: i64,
    pub gas_used: Option<i64>,
    pub hash: String,
    pub max_fee_per_gas: Option<i64>,
    pub max_priority_fee_per_gas: Option<i64>,
    pub method: Option<String>,
    pub nonce: i64,
    pub position: Option<i64>,
    pub priority_fee: Option<i64>,
    pub raw_input: String,
    pub result: String,
    pub revert_reason: Option<Any>,
    pub status: Option<String>,
    pub timestamp: Option<String>,
    pub to: crate::address_param::AddressParam,
    pub token_transfers: Option<Vec<crate::token_transfer::TokenTransfer<Any>>>,
    pub token_transfers_overflow: Option<bool>,
    pub tx_burnt_fee: Option<i64>,
    pub tx_tag: Option<String>,
    pub tx_types: Vec<String>,
    #[serde(rename = "type")]
    pub type_: Option<i64>,
    pub value: i64,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TransactionConfirmationDuration {}

impl<Any: Default> Transaction<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> TransactionBuilder<crate::generics::MissingConfirmationDuration, crate::generics::MissingConfirmations, crate::generics::MissingCreatedContract, crate::generics::MissingFee, crate::generics::MissingFrom, crate::generics::MissingGasLimit, crate::generics::MissingGasPrice, crate::generics::MissingHash, crate::generics::MissingNonce, crate::generics::MissingRawInput, crate::generics::MissingResult, crate::generics::MissingTo, crate::generics::MissingTxTypes, crate::generics::MissingValue, Any> {
        TransactionBuilder {
            body: Default::default(),
            _confirmation_duration: core::marker::PhantomData,
            _confirmations: core::marker::PhantomData,
            _created_contract: core::marker::PhantomData,
            _fee: core::marker::PhantomData,
            _from: core::marker::PhantomData,
            _gas_limit: core::marker::PhantomData,
            _gas_price: core::marker::PhantomData,
            _hash: core::marker::PhantomData,
            _nonce: core::marker::PhantomData,
            _raw_input: core::marker::PhantomData,
            _result: core::marker::PhantomData,
            _to: core::marker::PhantomData,
            _tx_types: core::marker::PhantomData,
            _value: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_main_page_txs() -> TransactionGetBuilder {
        TransactionGetBuilder
    }

    #[inline]
    pub fn get_tx() -> TransactionGetBuilder1<crate::generics::MissingTransactionHash> {
        TransactionGetBuilder1 {
            inner: Default::default(),
            _param_transaction_hash: core::marker::PhantomData,
        }
    }
}

impl<Any> Into<Transaction<Any>> for TransactionBuilder<crate::generics::ConfirmationDurationExists, crate::generics::ConfirmationsExists, crate::generics::CreatedContractExists, crate::generics::FeeExists, crate::generics::FromExists, crate::generics::GasLimitExists, crate::generics::GasPriceExists, crate::generics::HashExists, crate::generics::NonceExists, crate::generics::RawInputExists, crate::generics::ResultExists, crate::generics::ToExists, crate::generics::TxTypesExists, crate::generics::ValueExists, Any> {
    fn into(self) -> Transaction<Any> {
        self.body
    }
}

/// Builder for [`Transaction`](./struct.Transaction.html) object.
#[derive(Debug, Clone)]
pub struct TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, Fee, From, GasLimit, GasPrice, Hash, Nonce, RawInput, Result, To, TxTypes, Value, Any> {
    body: self::Transaction<Any>,
    _confirmation_duration: core::marker::PhantomData<ConfirmationDuration>,
    _confirmations: core::marker::PhantomData<Confirmations>,
    _created_contract: core::marker::PhantomData<CreatedContract>,
    _fee: core::marker::PhantomData<Fee>,
    _from: core::marker::PhantomData<From>,
    _gas_limit: core::marker::PhantomData<GasLimit>,
    _gas_price: core::marker::PhantomData<GasPrice>,
    _hash: core::marker::PhantomData<Hash>,
    _nonce: core::marker::PhantomData<Nonce>,
    _raw_input: core::marker::PhantomData<RawInput>,
    _result: core::marker::PhantomData<Result>,
    _to: core::marker::PhantomData<To>,
    _tx_types: core::marker::PhantomData<TxTypes>,
    _value: core::marker::PhantomData<Value>,
}

impl<ConfirmationDuration, Confirmations, CreatedContract, Fee, From, GasLimit, GasPrice, Hash, Nonce, RawInput, Result, To, TxTypes, Value, Any> TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, Fee, From, GasLimit, GasPrice, Hash, Nonce, RawInput, Result, To, TxTypes, Value, Any> {
    #[inline]
    pub fn base_fee_per_gas(mut self, value: impl Into<i64>) -> Self {
        self.body.base_fee_per_gas = Some(value.into());
        self
    }

    #[inline]
    pub fn block(mut self, value: impl Into<i32>) -> Self {
        self.body.block = Some(value.into());
        self
    }

    #[inline]
    pub fn confirmation_duration(mut self, value: crate::transaction::TransactionConfirmationDuration) -> TransactionBuilder<crate::generics::ConfirmationDurationExists, Confirmations, CreatedContract, Fee, From, GasLimit, GasPrice, Hash, Nonce, RawInput, Result, To, TxTypes, Value, Any> {
        self.body.confirmation_duration = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn confirmations(mut self, value: impl Into<i64>) -> TransactionBuilder<ConfirmationDuration, crate::generics::ConfirmationsExists, CreatedContract, Fee, From, GasLimit, GasPrice, Hash, Nonce, RawInput, Result, To, TxTypes, Value, Any> {
        self.body.confirmations = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn created_contract(mut self, value: crate::address_param::AddressParamBuilder<crate::generics::HashExists>) -> TransactionBuilder<ConfirmationDuration, Confirmations, crate::generics::CreatedContractExists, Fee, From, GasLimit, GasPrice, Hash, Nonce, RawInput, Result, To, TxTypes, Value, Any> {
        self.body.created_contract = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn decoded_input(mut self, value: crate::decoded_input::DecodedInputBuilder<crate::generics::MethodCallExists, crate::generics::MethodIdExists, crate::generics::ParametersExists>) -> Self {
        self.body.decoded_input = Some(value.into());
        self
    }

    #[inline]
    pub fn exchange_rate(mut self, value: impl Into<f64>) -> Self {
        self.body.exchange_rate = Some(value.into());
        self
    }

    #[inline]
    pub fn fee(mut self, value: crate::fee::FeeBuilder<crate::generics::TypeExists, crate::generics::ValueExists>) -> TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, crate::generics::FeeExists, From, GasLimit, GasPrice, Hash, Nonce, RawInput, Result, To, TxTypes, Value, Any> {
        self.body.fee = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn from(mut self, value: crate::address_param::AddressParamBuilder<crate::generics::HashExists>) -> TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, Fee, crate::generics::FromExists, GasLimit, GasPrice, Hash, Nonce, RawInput, Result, To, TxTypes, Value, Any> {
        self.body.from = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn gas_limit(mut self, value: impl Into<i64>) -> TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, Fee, From, crate::generics::GasLimitExists, GasPrice, Hash, Nonce, RawInput, Result, To, TxTypes, Value, Any> {
        self.body.gas_limit = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn gas_price(mut self, value: impl Into<i64>) -> TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, Fee, From, GasLimit, crate::generics::GasPriceExists, Hash, Nonce, RawInput, Result, To, TxTypes, Value, Any> {
        self.body.gas_price = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn gas_used(mut self, value: impl Into<i64>) -> Self {
        self.body.gas_used = Some(value.into());
        self
    }

    #[inline]
    pub fn hash(mut self, value: impl Into<String>) -> TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, Fee, From, GasLimit, GasPrice, crate::generics::HashExists, Nonce, RawInput, Result, To, TxTypes, Value, Any> {
        self.body.hash = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn max_fee_per_gas(mut self, value: impl Into<i64>) -> Self {
        self.body.max_fee_per_gas = Some(value.into());
        self
    }

    #[inline]
    pub fn max_priority_fee_per_gas(mut self, value: impl Into<i64>) -> Self {
        self.body.max_priority_fee_per_gas = Some(value.into());
        self
    }

    #[inline]
    pub fn method(mut self, value: impl Into<String>) -> Self {
        self.body.method = Some(value.into());
        self
    }

    #[inline]
    pub fn nonce(mut self, value: impl Into<i64>) -> TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, Fee, From, GasLimit, GasPrice, Hash, crate::generics::NonceExists, RawInput, Result, To, TxTypes, Value, Any> {
        self.body.nonce = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn position(mut self, value: impl Into<i64>) -> Self {
        self.body.position = Some(value.into());
        self
    }

    #[inline]
    pub fn priority_fee(mut self, value: impl Into<i64>) -> Self {
        self.body.priority_fee = Some(value.into());
        self
    }

    #[inline]
    pub fn raw_input(mut self, value: impl Into<String>) -> TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, Fee, From, GasLimit, GasPrice, Hash, Nonce, crate::generics::RawInputExists, Result, To, TxTypes, Value, Any> {
        self.body.raw_input = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn result(mut self, value: impl Into<String>) -> TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, Fee, From, GasLimit, GasPrice, Hash, Nonce, RawInput, crate::generics::ResultExists, To, TxTypes, Value, Any> {
        self.body.result = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn revert_reason(mut self, value: impl Into<Any>) -> Self {
        self.body.revert_reason = Some(value.into());
        self
    }

    #[inline]
    pub fn status(mut self, value: impl Into<String>) -> Self {
        self.body.status = Some(value.into());
        self
    }

    #[inline]
    pub fn timestamp(mut self, value: impl Into<String>) -> Self {
        self.body.timestamp = Some(value.into());
        self
    }

    #[inline]
    pub fn to(mut self, value: crate::address_param::AddressParamBuilder<crate::generics::HashExists>) -> TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, Fee, From, GasLimit, GasPrice, Hash, Nonce, RawInput, Result, crate::generics::ToExists, TxTypes, Value, Any> {
        self.body.to = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn token_transfers(mut self, value: impl Iterator<Item = crate::token_transfer::TokenTransferBuilder<crate::generics::FromExists, crate::generics::ToExists, crate::generics::TokenExists, crate::generics::TotalExists, crate::generics::TxHashExists, crate::generics::TypeExists, Any>>) -> Self {
        self.body.token_transfers = Some(value.map(|value| value.into()).collect::<Vec<_>>().into());
        self
    }

    #[inline]
    pub fn token_transfers_overflow(mut self, value: impl Into<bool>) -> Self {
        self.body.token_transfers_overflow = Some(value.into());
        self
    }

    #[inline]
    pub fn tx_burnt_fee(mut self, value: impl Into<i64>) -> Self {
        self.body.tx_burnt_fee = Some(value.into());
        self
    }

    #[inline]
    pub fn tx_tag(mut self, value: impl Into<String>) -> Self {
        self.body.tx_tag = Some(value.into());
        self
    }

    #[inline]
    pub fn tx_types(mut self, value: impl Iterator<Item = impl Into<String>>) -> TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, Fee, From, GasLimit, GasPrice, Hash, Nonce, RawInput, Result, To, crate::generics::TxTypesExists, Value, Any> {
        self.body.tx_types = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<i64>) -> Self {
        self.body.type_ = Some(value.into());
        self
    }

    #[inline]
    pub fn value(mut self, value: impl Into<i64>) -> TransactionBuilder<ConfirmationDuration, Confirmations, CreatedContract, Fee, From, GasLimit, GasPrice, Hash, Nonce, RawInput, Result, To, TxTypes, crate::generics::ValueExists, Any> {
        self.body.value = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`Transaction::get_main_page_txs`](./struct.Transaction.html#method.get_main_page_txs) method for a `GET` operation associated with `Transaction`.
#[derive(Debug, Clone)]
pub struct TransactionGetBuilder;


impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for TransactionGetBuilder {
    type Output = Vec<Transaction<serde_yaml::Value>>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/main-page/transactions".into()
    }
}

/// Builder created by [`Transaction::get_tx`](./struct.Transaction.html#method.get_tx) method for a `GET` operation associated with `Transaction`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct TransactionGetBuilder1<TransactionHash> {
    inner: TransactionGetBuilder1Container,
    _param_transaction_hash: core::marker::PhantomData<TransactionHash>,
}

#[derive(Debug, Default, Clone)]
struct TransactionGetBuilder1Container {
    param_transaction_hash: Option<String>,
}

impl<TransactionHash> TransactionGetBuilder1<TransactionHash> {
    /// Transaction hash
    #[inline]
    pub fn transaction_hash(mut self, value: impl Into<String>) -> TransactionGetBuilder1<crate::generics::TransactionHashExists> {
        self.inner.param_transaction_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for TransactionGetBuilder1<crate::generics::TransactionHashExists> {
    type Output = Transaction<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/transactions/{transaction_hash}", transaction_hash=self.inner.param_transaction_hash.as_ref().expect("missing parameter transaction_hash?")).into()
    }
}

