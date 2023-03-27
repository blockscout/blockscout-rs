#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AddressParam {
    pub hash: String,
    pub implementation_name: Option<String>,
    pub is_contract: Option<bool>,
    pub is_verified: Option<bool>,
    pub name: Option<String>,
    pub private_tags: Option<Vec<crate::address_tag::AddressTag>>,
    pub public_tags: Option<Vec<crate::address_tag::AddressTag>>,
    pub watchlist_names: Option<Vec<crate::watchlist_name::WatchlistName>>,
}

impl AddressParam {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> AddressParamBuilder<crate::generics::MissingHash> {
        AddressParamBuilder {
            body: Default::default(),
            _hash: core::marker::PhantomData,
        }
    }
}

impl Into<AddressParam> for AddressParamBuilder<crate::generics::HashExists> {
    fn into(self) -> AddressParam {
        self.body
    }
}

/// Builder for [`AddressParam`](./struct.AddressParam.html) object.
#[derive(Debug, Clone)]
pub struct AddressParamBuilder<Hash> {
    body: self::AddressParam,
    _hash: core::marker::PhantomData<Hash>,
}

impl<Hash> AddressParamBuilder<Hash> {
    #[inline]
    pub fn hash(mut self, value: impl Into<String>) -> AddressParamBuilder<crate::generics::HashExists> {
        self.body.hash = value.into();
        unsafe { std::mem::transmute(self) }
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
    pub fn watchlist_names(mut self, value: impl Iterator<Item = crate::watchlist_name::WatchlistNameBuilder<crate::generics::DisplayNameExists, crate::generics::LabelExists>>) -> Self {
        self.body.watchlist_names = Some(value.map(|value| value.into()).collect::<Vec<_>>().into());
        self
    }
}
