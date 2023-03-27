#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RevertReasonAsMap {
    pub raw: String,
}

impl RevertReasonAsMap {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> RevertReasonAsMapBuilder<crate::generics::MissingRaw> {
        RevertReasonAsMapBuilder {
            body: Default::default(),
            _raw: core::marker::PhantomData,
        }
    }
}

impl Into<RevertReasonAsMap> for RevertReasonAsMapBuilder<crate::generics::RawExists> {
    fn into(self) -> RevertReasonAsMap {
        self.body
    }
}

/// Builder for [`RevertReasonAsMap`](./struct.RevertReasonAsMap.html) object.
#[derive(Debug, Clone)]
pub struct RevertReasonAsMapBuilder<Raw> {
    body: self::RevertReasonAsMap,
    _raw: core::marker::PhantomData<Raw>,
}

impl<Raw> RevertReasonAsMapBuilder<Raw> {
    #[inline]
    pub fn raw(mut self, value: impl Into<String>) -> RevertReasonAsMapBuilder<crate::generics::RawExists> {
        self.body.raw = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
