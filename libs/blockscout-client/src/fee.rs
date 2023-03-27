#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Fee {
    #[serde(rename = "type")]
    pub type_: String,
    pub value: String,
}

impl Fee {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> FeeBuilder<crate::generics::MissingType, crate::generics::MissingValue> {
        FeeBuilder {
            body: Default::default(),
            _type: core::marker::PhantomData,
            _value: core::marker::PhantomData,
        }
    }
}

impl Into<Fee> for FeeBuilder<crate::generics::TypeExists, crate::generics::ValueExists> {
    fn into(self) -> Fee {
        self.body
    }
}

/// Builder for [`Fee`](./struct.Fee.html) object.
#[derive(Debug, Clone)]
pub struct FeeBuilder<Type, Value> {
    body: self::Fee,
    _type: core::marker::PhantomData<Type>,
    _value: core::marker::PhantomData<Value>,
}

impl<Type, Value> FeeBuilder<Type, Value> {
    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> FeeBuilder<crate::generics::TypeExists, Value> {
        self.body.type_ = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn value(mut self, value: impl Into<String>) -> FeeBuilder<Type, crate::generics::ValueExists> {
        self.body.value = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
