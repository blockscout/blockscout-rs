#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DecodedInput {
    pub method_call: String,
    pub method_id: String,
    pub parameters: Vec<crate::decoded_input_parameter::DecodedInputParameter>,
}

impl DecodedInput {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> DecodedInputBuilder<crate::generics::MissingMethodCall, crate::generics::MissingMethodId, crate::generics::MissingParameters> {
        DecodedInputBuilder {
            body: Default::default(),
            _method_call: core::marker::PhantomData,
            _method_id: core::marker::PhantomData,
            _parameters: core::marker::PhantomData,
        }
    }
}

impl Into<DecodedInput> for DecodedInputBuilder<crate::generics::MethodCallExists, crate::generics::MethodIdExists, crate::generics::ParametersExists> {
    fn into(self) -> DecodedInput {
        self.body
    }
}

/// Builder for [`DecodedInput`](./struct.DecodedInput.html) object.
#[derive(Debug, Clone)]
pub struct DecodedInputBuilder<MethodCall, MethodId, Parameters> {
    body: self::DecodedInput,
    _method_call: core::marker::PhantomData<MethodCall>,
    _method_id: core::marker::PhantomData<MethodId>,
    _parameters: core::marker::PhantomData<Parameters>,
}

impl<MethodCall, MethodId, Parameters> DecodedInputBuilder<MethodCall, MethodId, Parameters> {
    #[inline]
    pub fn method_call(mut self, value: impl Into<String>) -> DecodedInputBuilder<crate::generics::MethodCallExists, MethodId, Parameters> {
        self.body.method_call = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn method_id(mut self, value: impl Into<String>) -> DecodedInputBuilder<MethodCall, crate::generics::MethodIdExists, Parameters> {
        self.body.method_id = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn parameters(mut self, value: impl Iterator<Item = crate::decoded_input_parameter::DecodedInputParameterBuilder<crate::generics::NameExists, crate::generics::TypeExists, crate::generics::ValueExists>>) -> DecodedInputBuilder<MethodCall, MethodId, crate::generics::ParametersExists> {
        self.body.parameters = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }
}
