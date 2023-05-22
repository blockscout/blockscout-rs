#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetConfigJsonRpcUrlResponse {
    pub json_rpc_url: String,
}

impl GetConfigJsonRpcUrlResponse {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetConfigJsonRpcUrlResponseBuilder<crate::generics::MissingJsonRpcUrl> {
        GetConfigJsonRpcUrlResponseBuilder {
            body: Default::default(),
            _json_rpc_url: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_json_rpc_url() -> GetConfigJsonRpcUrlResponseGetBuilder {
        GetConfigJsonRpcUrlResponseGetBuilder
    }
}

impl Into<GetConfigJsonRpcUrlResponse> for GetConfigJsonRpcUrlResponseBuilder<crate::generics::JsonRpcUrlExists> {
    fn into(self) -> GetConfigJsonRpcUrlResponse {
        self.body
    }
}

/// Builder for [`GetConfigJsonRpcUrlResponse`](./struct.GetConfigJsonRpcUrlResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetConfigJsonRpcUrlResponseBuilder<JsonRpcUrl> {
    body: self::GetConfigJsonRpcUrlResponse,
    _json_rpc_url: core::marker::PhantomData<JsonRpcUrl>,
}

impl<JsonRpcUrl> GetConfigJsonRpcUrlResponseBuilder<JsonRpcUrl> {
    #[inline]
    pub fn json_rpc_url(mut self, value: impl Into<String>) -> GetConfigJsonRpcUrlResponseBuilder<crate::generics::JsonRpcUrlExists> {
        self.body.json_rpc_url = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetConfigJsonRpcUrlResponse::get_json_rpc_url`](./struct.GetConfigJsonRpcUrlResponse.html#method.get_json_rpc_url) method for a `GET` operation associated with `GetConfigJsonRpcUrlResponse`.
#[derive(Debug, Clone)]
pub struct GetConfigJsonRpcUrlResponseGetBuilder;


impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetConfigJsonRpcUrlResponseGetBuilder {
    type Output = GetConfigJsonRpcUrlResponse;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/config/json-rpc-url".into()
    }
}
