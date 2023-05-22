#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DecodedInputLog {
    pub method_call: String,
    pub method_id: String,
    pub parameters: Vec<crate::decoded_input_log_parameter::DecodedInputLogParameter>,
}

impl DecodedInputLog {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> DecodedInputLogBuilder<crate::generics::MissingMethodCall, crate::generics::MissingMethodId, crate::generics::MissingParameters> {
        DecodedInputLogBuilder {
            body: Default::default(),
            _method_call: core::marker::PhantomData,
            _method_id: core::marker::PhantomData,
            _parameters: core::marker::PhantomData,
        }
    }
}

impl Into<DecodedInputLog> for DecodedInputLogBuilder<crate::generics::MethodCallExists, crate::generics::MethodIdExists, crate::generics::ParametersExists> {
    fn into(self) -> DecodedInputLog {
        self.body
    }
}

/// Builder for [`DecodedInputLog`](./struct.DecodedInputLog.html) object.
#[derive(Debug, Clone)]
pub struct DecodedInputLogBuilder<MethodCall, MethodId, Parameters> {
    body: self::DecodedInputLog,
    _method_call: core::marker::PhantomData<MethodCall>,
    _method_id: core::marker::PhantomData<MethodId>,
    _parameters: core::marker::PhantomData<Parameters>,
}

impl<MethodCall, MethodId, Parameters> DecodedInputLogBuilder<MethodCall, MethodId, Parameters> {
    #[inline]
    pub fn method_call(mut self, value: impl Into<String>) -> DecodedInputLogBuilder<crate::generics::MethodCallExists, MethodId, Parameters> {
        self.body.method_call = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn method_id(mut self, value: impl Into<String>) -> DecodedInputLogBuilder<MethodCall, crate::generics::MethodIdExists, Parameters> {
        self.body.method_id = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn parameters(mut self, value: impl Iterator<Item = crate::decoded_input_log_parameter::DecodedInputLogParameterBuilder<crate::generics::IndexedExists, crate::generics::NameExists, crate::generics::TypeExists, crate::generics::ValueExists>>) -> DecodedInputLogBuilder<MethodCall, MethodId, crate::generics::ParametersExists> {
        self.body.parameters = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }
}
