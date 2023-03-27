#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DecodedInputParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub value: String,
}

impl DecodedInputParameter {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> DecodedInputParameterBuilder<crate::generics::MissingName, crate::generics::MissingType, crate::generics::MissingValue> {
        DecodedInputParameterBuilder {
            body: Default::default(),
            _name: core::marker::PhantomData,
            _type: core::marker::PhantomData,
            _value: core::marker::PhantomData,
        }
    }
}

impl Into<DecodedInputParameter> for DecodedInputParameterBuilder<crate::generics::NameExists, crate::generics::TypeExists, crate::generics::ValueExists> {
    fn into(self) -> DecodedInputParameter {
        self.body
    }
}

/// Builder for [`DecodedInputParameter`](./struct.DecodedInputParameter.html) object.
#[derive(Debug, Clone)]
pub struct DecodedInputParameterBuilder<Name, Type, Value> {
    body: self::DecodedInputParameter,
    _name: core::marker::PhantomData<Name>,
    _type: core::marker::PhantomData<Type>,
    _value: core::marker::PhantomData<Value>,
}

impl<Name, Type, Value> DecodedInputParameterBuilder<Name, Type, Value> {
    #[inline]
    pub fn name(mut self, value: impl Into<String>) -> DecodedInputParameterBuilder<crate::generics::NameExists, Type, Value> {
        self.body.name = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> DecodedInputParameterBuilder<Name, crate::generics::TypeExists, Value> {
        self.body.type_ = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn value(mut self, value: impl Into<String>) -> DecodedInputParameterBuilder<Name, Type, crate::generics::ValueExists> {
        self.body.value = value.into();
        unsafe { std::mem::transmute(self) }
    }
}
