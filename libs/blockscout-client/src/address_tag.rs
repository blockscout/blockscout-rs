#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AddressTag {
    pub address_hash: String,
    pub display_name: String,
    pub label: String,
}

impl AddressTag {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> AddressTagBuilder<crate::generics::MissingAddressHash, crate::generics::MissingDisplayName, crate::generics::MissingLabel> {
        AddressTagBuilder {
            body: Default::default(),
            _address_hash: core::marker::PhantomData,
            _display_name: core::marker::PhantomData,
            _label: core::marker::PhantomData,
        }
    }
}

impl Into<AddressTag> for AddressTagBuilder<crate::generics::AddressHashExists, crate::generics::DisplayNameExists, crate::generics::LabelExists> {
    fn into(self) -> AddressTag {
        self.body
    }
}

/// Builder for [`AddressTag`](./struct.AddressTag.html) object.
#[derive(Debug, Clone)]
pub struct AddressTagBuilder<AddressHash, DisplayName, Label> {
    body: self::AddressTag,
    _address_hash: core::marker::PhantomData<AddressHash>,
    _display_name: core::marker::PhantomData<DisplayName>,
    _label: core::marker::PhantomData<Label>,
}

impl<AddressHash, DisplayName, Label> AddressTagBuilder<AddressHash, DisplayName, Label> {
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> AddressTagBuilder<crate::generics::AddressHashExists, DisplayName, Label> {
        self.body.address_hash = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn display_name(mut self, value: impl Into<String>) -> AddressTagBuilder<AddressHash, crate::generics::DisplayNameExists, Label> {
        self.body.display_name = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn label(mut self, value: impl Into<String>) -> AddressTagBuilder<AddressHash, DisplayName, crate::generics::LabelExists> {
        self.body.label = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
