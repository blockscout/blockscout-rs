#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WatchlistName {
    pub display_name: String,
    pub label: String,
}

impl WatchlistName {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> WatchlistNameBuilder<crate::generics::MissingDisplayName, crate::generics::MissingLabel> {
        WatchlistNameBuilder {
            body: Default::default(),
            _display_name: core::marker::PhantomData,
            _label: core::marker::PhantomData,
        }
    }
}

impl Into<WatchlistName> for WatchlistNameBuilder<crate::generics::DisplayNameExists, crate::generics::LabelExists> {
    fn into(self) -> WatchlistName {
        self.body
    }
}

/// Builder for [`WatchlistName`](./struct.WatchlistName.html) object.
#[derive(Debug, Clone)]
pub struct WatchlistNameBuilder<DisplayName, Label> {
    body: self::WatchlistName,
    _display_name: core::marker::PhantomData<DisplayName>,
    _label: core::marker::PhantomData<Label>,
}

impl<DisplayName, Label> WatchlistNameBuilder<DisplayName, Label> {
    #[inline]
    pub fn display_name(mut self, value: impl Into<String>) -> WatchlistNameBuilder<crate::generics::DisplayNameExists, Label> {
        self.body.display_name = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn label(mut self, value: impl Into<String>) -> WatchlistNameBuilder<DisplayName, crate::generics::LabelExists> {
        self.body.label = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
