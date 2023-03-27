#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ExtendedRevertReasonAsMap {
    pub code: i64,
    pub message: String,
    pub raw: String,
}

impl ExtendedRevertReasonAsMap {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> ExtendedRevertReasonAsMapBuilder<crate::generics::MissingCode, crate::generics::MissingMessage, crate::generics::MissingRaw> {
        ExtendedRevertReasonAsMapBuilder {
            body: Default::default(),
            _code: core::marker::PhantomData,
            _message: core::marker::PhantomData,
            _raw: core::marker::PhantomData,
        }
    }
}

impl Into<ExtendedRevertReasonAsMap> for ExtendedRevertReasonAsMapBuilder<crate::generics::CodeExists, crate::generics::MessageExists, crate::generics::RawExists> {
    fn into(self) -> ExtendedRevertReasonAsMap {
        self.body
    }
}

/// Builder for [`ExtendedRevertReasonAsMap`](./struct.ExtendedRevertReasonAsMap.html) object.
#[derive(Debug, Clone)]
pub struct ExtendedRevertReasonAsMapBuilder<Code, Message, Raw> {
    body: self::ExtendedRevertReasonAsMap,
    _code: core::marker::PhantomData<Code>,
    _message: core::marker::PhantomData<Message>,
    _raw: core::marker::PhantomData<Raw>,
}

impl<Code, Message, Raw> ExtendedRevertReasonAsMapBuilder<Code, Message, Raw> {
    #[inline]
    pub fn code(mut self, value: impl Into<i64>) -> ExtendedRevertReasonAsMapBuilder<crate::generics::CodeExists, Message, Raw> {
        self.body.code = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn message(mut self, value: impl Into<String>) -> ExtendedRevertReasonAsMapBuilder<Code, crate::generics::MessageExists, Raw> {
        self.body.message = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn raw(mut self, value: impl Into<String>) -> ExtendedRevertReasonAsMapBuilder<Code, Message, crate::generics::RawExists> {
        self.body.raw = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
