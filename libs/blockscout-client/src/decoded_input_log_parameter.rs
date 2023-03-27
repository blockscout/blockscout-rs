#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DecodedInputLogParameter {
    #[serde(rename = "indexed?")]
    pub indexed: bool,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub value: String,
}

impl DecodedInputLogParameter {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> DecodedInputLogParameterBuilder<crate::generics::MissingIndexed, crate::generics::MissingName, crate::generics::MissingType, crate::generics::MissingValue> {
        DecodedInputLogParameterBuilder {
            body: Default::default(),
            _indexed: core::marker::PhantomData,
            _name: core::marker::PhantomData,
            _type: core::marker::PhantomData,
            _value: core::marker::PhantomData,
        }
    }
}

impl Into<DecodedInputLogParameter> for DecodedInputLogParameterBuilder<crate::generics::IndexedExists, crate::generics::NameExists, crate::generics::TypeExists, crate::generics::ValueExists> {
    fn into(self) -> DecodedInputLogParameter {
        self.body
    }
}

/// Builder for [`DecodedInputLogParameter`](./struct.DecodedInputLogParameter.html) object.
#[derive(Debug, Clone)]
pub struct DecodedInputLogParameterBuilder<Indexed, Name, Type, Value> {
    body: self::DecodedInputLogParameter,
    _indexed: core::marker::PhantomData<Indexed>,
    _name: core::marker::PhantomData<Name>,
    _type: core::marker::PhantomData<Type>,
    _value: core::marker::PhantomData<Value>,
}

impl<Indexed, Name, Type, Value> DecodedInputLogParameterBuilder<Indexed, Name, Type, Value> {
    #[inline]
    pub fn indexed(mut self, value: impl Into<bool>) -> DecodedInputLogParameterBuilder<crate::generics::IndexedExists, Name, Type, Value> {
        self.body.indexed = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn name(mut self, value: impl Into<String>) -> DecodedInputLogParameterBuilder<Indexed, crate::generics::NameExists, Type, Value> {
        self.body.name = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> DecodedInputLogParameterBuilder<Indexed, Name, crate::generics::TypeExists, Value> {
        self.body.type_ = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn value(mut self, value: impl Into<String>) -> DecodedInputLogParameterBuilder<Indexed, Name, Type, crate::generics::ValueExists> {
        self.body.value = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
