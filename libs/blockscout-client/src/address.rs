#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Address<Any> {
    pub block_number_balance_updated_at: Option<i64>,
    pub coin_balance: Option<String>,
    pub creation_tx_hash: Option<String>,
    pub creator_address_hash: Option<String>,
    pub exchange_rate: Option<String>,
    pub hash: String,
    pub implementation_address: Option<String>,
    pub implementation_name: Option<String>,
    pub is_contract: Option<bool>,
    pub is_verified: Option<bool>,
    pub name: Option<String>,
    pub private_tags: Option<Vec<crate::address_tag::AddressTag>>,
    pub public_tags: Option<Vec<crate::address_tag::AddressTag>>,
    pub token: Option<Any>,
    pub watchlist_names: Option<Vec<crate::watchlist_name::WatchlistName>>,
}

impl<Any: Default> Address<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> AddressBuilder<crate::generics::MissingHash, Any> {
        AddressBuilder {
            body: Default::default(),
            _hash: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_address() -> AddressGetBuilder<crate::generics::MissingAddressHash> {
        AddressGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }
}

impl<Any> Into<Address<Any>> for AddressBuilder<crate::generics::HashExists, Any> {
    fn into(self) -> Address<Any> {
        self.body
    }
}

/// Builder for [`Address`](./struct.Address.html) object.
#[derive(Debug, Clone)]
pub struct AddressBuilder<Hash, Any> {
    body: self::Address<Any>,
    _hash: core::marker::PhantomData<Hash>,
}

impl<Hash, Any> AddressBuilder<Hash, Any> {
    #[inline]
    pub fn block_number_balance_updated_at(mut self, value: impl Into<i64>) -> Self {
        self.body.block_number_balance_updated_at = Some(value.into());
        self
    }

    #[inline]
    pub fn coin_balance(mut self, value: impl Into<String>) -> Self {
        self.body.coin_balance = Some(value.into());
        self
    }

    #[inline]
    pub fn creation_tx_hash(mut self, value: impl Into<String>) -> Self {
        self.body.creation_tx_hash = Some(value.into());
        self
    }

    #[inline]
    pub fn creator_address_hash(mut self, value: impl Into<String>) -> Self {
        self.body.creator_address_hash = Some(value.into());
        self
    }

    #[inline]
    pub fn exchange_rate(mut self, value: impl Into<String>) -> Self {
        self.body.exchange_rate = Some(value.into());
        self
    }

    #[inline]
    pub fn hash(mut self, value: impl Into<String>) -> AddressBuilder<crate::generics::HashExists, Any> {
        self.body.hash = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn implementation_address(mut self, value: impl Into<String>) -> Self {
        self.body.implementation_address = Some(value.into());
        self
    }

    #[inline]
    pub fn implementation_name(mut self, value: impl Into<String>) -> Self {
        self.body.implementation_name = Some(value.into());
        self
    }

    #[inline]
    pub fn is_contract(mut self, value: impl Into<bool>) -> Self {
        self.body.is_contract = Some(value.into());
        self
    }

    #[inline]
    pub fn is_verified(mut self, value: impl Into<bool>) -> Self {
        self.body.is_verified = Some(value.into());
        self
    }

    #[inline]
    pub fn name(mut self, value: impl Into<String>) -> Self {
        self.body.name = Some(value.into());
        self
    }

    #[inline]
    pub fn private_tags(mut self, value: impl Iterator<Item = crate::address_tag::AddressTagBuilder<crate::generics::AddressHashExists, crate::generics::DisplayNameExists, crate::generics::LabelExists>>) -> Self {
        self.body.private_tags = Some(value.map(|value| value.into()).collect::<Vec<_>>().into());
        self
    }

    #[inline]
    pub fn public_tags(mut self, value: impl Iterator<Item = crate::address_tag::AddressTagBuilder<crate::generics::AddressHashExists, crate::generics::DisplayNameExists, crate::generics::LabelExists>>) -> Self {
        self.body.public_tags = Some(value.map(|value| value.into()).collect::<Vec<_>>().into());
        self
    }

    #[inline]
    pub fn token(mut self, value: impl Into<Any>) -> Self {
        self.body.token = Some(value.into());
        self
    }

    #[inline]
    pub fn watchlist_names(mut self, value: impl Iterator<Item = crate::watchlist_name::WatchlistNameBuilder<crate::generics::DisplayNameExists, crate::generics::LabelExists>>) -> Self {
        self.body.watchlist_names = Some(value.map(|value| value.into()).collect::<Vec<_>>().into());
        self
    }
}

/// Builder created by [`Address::get_address`](./struct.Address.html#method.get_address) method for a `GET` operation associated with `Address`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct AddressGetBuilder<AddressHash> {
    inner: AddressGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct AddressGetBuilderContainer {
    param_address_hash: Option<String>,
}

impl<AddressHash> AddressGetBuilder<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> AddressGetBuilder<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for AddressGetBuilder<crate::generics::AddressHashExists> {
    type Output = Address<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/addresses/{address_hash}", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}
