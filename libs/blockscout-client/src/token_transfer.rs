#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TokenTransfer<Any> {
    pub block_hash: Option<String>,
    pub from: crate::address_param::AddressParam,
    pub method: Option<String>,
    pub timestamp: Option<String>,
    pub to: crate::address_param::AddressParam,
    pub token: Any,
    pub total: Any,
    pub tx_hash: String,
    #[serde(rename = "type")]
    pub type_: String,
}

impl<Any: Default> TokenTransfer<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> TokenTransferBuilder<crate::generics::MissingFrom, crate::generics::MissingTo, crate::generics::MissingToken, crate::generics::MissingTotal, crate::generics::MissingTxHash, crate::generics::MissingType, Any> {
        TokenTransferBuilder {
            body: Default::default(),
            _from: core::marker::PhantomData,
            _to: core::marker::PhantomData,
            _token: core::marker::PhantomData,
            _total: core::marker::PhantomData,
            _tx_hash: core::marker::PhantomData,
            _type: core::marker::PhantomData,
        }
    }
}

impl<Any> Into<TokenTransfer<Any>> for TokenTransferBuilder<crate::generics::FromExists, crate::generics::ToExists, crate::generics::TokenExists, crate::generics::TotalExists, crate::generics::TxHashExists, crate::generics::TypeExists, Any> {
    fn into(self) -> TokenTransfer<Any> {
        self.body
    }
}

/// Builder for [`TokenTransfer`](./struct.TokenTransfer.html) object.
#[derive(Debug, Clone)]
pub struct TokenTransferBuilder<From, To, Token, Total, TxHash, Type, Any> {
    body: self::TokenTransfer<Any>,
    _from: core::marker::PhantomData<From>,
    _to: core::marker::PhantomData<To>,
    _token: core::marker::PhantomData<Token>,
    _total: core::marker::PhantomData<Total>,
    _tx_hash: core::marker::PhantomData<TxHash>,
    _type: core::marker::PhantomData<Type>,
}

impl<From, To, Token, Total, TxHash, Type, Any> TokenTransferBuilder<From, To, Token, Total, TxHash, Type, Any> {
    #[inline]
    pub fn block_hash(mut self, value: impl Into<String>) -> Self {
        self.body.block_hash = Some(value.into());
        self
    }

    #[inline]
    pub fn from(mut self, value: crate::address_param::AddressParamBuilder<crate::generics::HashExists>) -> TokenTransferBuilder<crate::generics::FromExists, To, Token, Total, TxHash, Type, Any> {
        self.body.from = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn method(mut self, value: impl Into<String>) -> Self {
        self.body.method = Some(value.into());
        self
    }

    #[inline]
    pub fn timestamp(mut self, value: impl Into<String>) -> Self {
        self.body.timestamp = Some(value.into());
        self
    }

    #[inline]
    pub fn to(mut self, value: crate::address_param::AddressParamBuilder<crate::generics::HashExists>) -> TokenTransferBuilder<From, crate::generics::ToExists, Token, Total, TxHash, Type, Any> {
        self.body.to = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn token(mut self, value: impl Into<Any>) -> TokenTransferBuilder<From, To, crate::generics::TokenExists, Total, TxHash, Type, Any> {
        self.body.token = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn total(mut self, value: impl Into<Any>) -> TokenTransferBuilder<From, To, Token, crate::generics::TotalExists, TxHash, Type, Any> {
        self.body.total = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn tx_hash(mut self, value: impl Into<String>) -> TokenTransferBuilder<From, To, Token, Total, crate::generics::TxHashExists, Type, Any> {
        self.body.tx_hash = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> TokenTransferBuilder<From, To, Token, Total, TxHash, crate::generics::TypeExists, Any> {
        self.body.type_ = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
