#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TokenBalance {
    pub token: crate::token::Token,
    pub token_id: String,
    pub value: String,
}

impl TokenBalance {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> TokenBalanceBuilder<crate::generics::MissingToken, crate::generics::MissingTokenId, crate::generics::MissingValue> {
        TokenBalanceBuilder {
            body: Default::default(),
            _token: core::marker::PhantomData,
            _token_id: core::marker::PhantomData,
            _value: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_address_token_balances() -> TokenBalanceGetBuilder<crate::generics::MissingAddressHash> {
        TokenBalanceGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl Into<TokenBalance> for TokenBalanceBuilder<crate::generics::TokenExists, crate::generics::TokenIdExists, crate::generics::ValueExists> {
    fn into(self) -> TokenBalance {
        self.body
    }
}

/// Builder for [`TokenBalance`](./struct.TokenBalance.html) object.
#[derive(Debug, Clone)]
pub struct TokenBalanceBuilder<Token, TokenId, Value> {
    body: self::TokenBalance,
    _token: core::marker::PhantomData<Token>,
    _token_id: core::marker::PhantomData<TokenId>,
    _value: core::marker::PhantomData<Value>,
}

impl<Token, TokenId, Value> TokenBalanceBuilder<Token, TokenId, Value> {
    #[inline]
    pub fn token(mut self, value: crate::token::TokenBuilder<crate::generics::AddressExists, crate::generics::DecimalsExists, crate::generics::ExchangeRateExists, crate::generics::HoldersExists, crate::generics::NameExists, crate::generics::SymbolExists, crate::generics::TotalSupplyExists, crate::generics::TypeExists>) -> TokenBalanceBuilder<crate::generics::TokenExists, TokenId, Value> {
        self.body.token = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn token_id(mut self, value: impl Into<String>) -> TokenBalanceBuilder<Token, crate::generics::TokenIdExists, Value> {
        self.body.token_id = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn value(mut self, value: impl Into<String>) -> TokenBalanceBuilder<Token, TokenId, crate::generics::ValueExists> {
        self.body.value = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`TokenBalance::get_address_token_balances`](./struct.TokenBalance.html#method.get_address_token_balances) method for a `GET` operation associated with `TokenBalance`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct TokenBalanceGetBuilder<AddressHash> {
    inner: TokenBalanceGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct TokenBalanceGetBuilderContainer {
    param_address_hash: Option<String>,
}

impl<AddressHash> TokenBalanceGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> TokenBalanceGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for TokenBalanceGetBuilder<crate::generics::AddressHashExists> {
    type Output = Vec<TokenBalance>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/addresses/{address_hash}/token-balances", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}
