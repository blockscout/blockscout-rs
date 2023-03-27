#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Token {
    pub address: String,
    pub decimals: String,
    pub exchange_rate: String,
    pub holders: i64,
    pub name: String,
    pub symbol: String,
    pub total_supply: String,
    #[serde(rename = "type")]
    pub type_: String,
}

impl Token {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> TokenBuilder<crate::generics::MissingAddress, crate::generics::MissingDecimals, crate::generics::MissingExchangeRate, crate::generics::MissingHolders, crate::generics::MissingName, crate::generics::MissingSymbol, crate::generics::MissingTotalSupply, crate::generics::MissingType> {
        TokenBuilder {
            body: Default::default(),
            _address: core::marker::PhantomData,
            _decimals: core::marker::PhantomData,
            _exchange_rate: core::marker::PhantomData,
            _holders: core::marker::PhantomData,
            _name: core::marker::PhantomData,
            _symbol: core::marker::PhantomData,
            _total_supply: core::marker::PhantomData,
            _type: core::marker::PhantomData,
        }
    }
}

impl Into<Token> for TokenBuilder<crate::generics::AddressExists, crate::generics::DecimalsExists, crate::generics::ExchangeRateExists, crate::generics::HoldersExists, crate::generics::NameExists, crate::generics::SymbolExists, crate::generics::TotalSupplyExists, crate::generics::TypeExists> {
    fn into(self) -> Token {
        self.body
    }
}

/// Builder for [`Token`](./struct.Token.html) object.
#[derive(Debug, Clone)]
pub struct TokenBuilder<Address, Decimals, ExchangeRate, Holders, Name, Symbol, TotalSupply, Type> {
    body: self::Token,
    _address: core::marker::PhantomData<Address>,
    _decimals: core::marker::PhantomData<Decimals>,
    _exchange_rate: core::marker::PhantomData<ExchangeRate>,
    _holders: core::marker::PhantomData<Holders>,
    _name: core::marker::PhantomData<Name>,
    _symbol: core::marker::PhantomData<Symbol>,
    _total_supply: core::marker::PhantomData<TotalSupply>,
    _type: core::marker::PhantomData<Type>,
}

impl<Address, Decimals, ExchangeRate, Holders, Name, Symbol, TotalSupply, Type> TokenBuilder<Address, Decimals, ExchangeRate, Holders, Name, Symbol, TotalSupply, Type> {
    #[inline]
    pub fn address(mut self, value: impl Into<String>) -> TokenBuilder<crate::generics::AddressExists, Decimals, ExchangeRate, Holders, Name, Symbol, TotalSupply, Type> {
        self.body.address = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn decimals(mut self, value: impl Into<String>) -> TokenBuilder<Address, crate::generics::DecimalsExists, ExchangeRate, Holders, Name, Symbol, TotalSupply, Type> {
        self.body.decimals = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn exchange_rate(mut self, value: impl Into<String>) -> TokenBuilder<Address, Decimals, crate::generics::ExchangeRateExists, Holders, Name, Symbol, TotalSupply, Type> {
        self.body.exchange_rate = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn holders(mut self, value: impl Into<i64>) -> TokenBuilder<Address, Decimals, ExchangeRate, crate::generics::HoldersExists, Name, Symbol, TotalSupply, Type> {
        self.body.holders = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn name(mut self, value: impl Into<String>) -> TokenBuilder<Address, Decimals, ExchangeRate, Holders, crate::generics::NameExists, Symbol, TotalSupply, Type> {
        self.body.name = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn symbol(mut self, value: impl Into<String>) -> TokenBuilder<Address, Decimals, ExchangeRate, Holders, Name, crate::generics::SymbolExists, TotalSupply, Type> {
        self.body.symbol = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn total_supply(mut self, value: impl Into<String>) -> TokenBuilder<Address, Decimals, ExchangeRate, Holders, Name, Symbol, crate::generics::TotalSupplyExists, Type> {
        self.body.total_supply = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> TokenBuilder<Address, Decimals, ExchangeRate, Holders, Name, Symbol, TotalSupply, crate::generics::TypeExists> {
        self.body.type_ = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
