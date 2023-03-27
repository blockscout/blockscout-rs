#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTokensResponse<Any> {
    pub items: Vec<Any>,
    pub next_page_params: crate::get_tokens_response::GetTokensResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTokensResponseNextPageParams {}

impl<Any: Default> GetTokensResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetTokensResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams, Any> {
        GetTokensResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_tokens_list() -> GetTokensResponseGetBuilder {
        GetTokensResponseGetBuilder {
            param_q: None,
            param_type: None,
        }
    }
}

impl<Any> Into<GetTokensResponse<Any>> for GetTokensResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists, Any> {
    fn into(self) -> GetTokensResponse<Any> {
        self.body
    }
}

/// Builder for [`GetTokensResponse`](./struct.GetTokensResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetTokensResponseBuilder<Items, NextPageParams, Any> {
    body: self::GetTokensResponse<Any>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams, Any> GetTokensResponseBuilder<Items, NextPageParams, Any> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = impl Into<Any>>) -> GetTokensResponseBuilder<crate::generics::ItemsExists, NextPageParams, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_tokens_response::GetTokensResponseNextPageParams) -> GetTokensResponseBuilder<Items, crate::generics::NextPageParamsExists, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetTokensResponse::get_tokens_list`](./struct.GetTokensResponse.html#method.get_tokens_list) method for a `GET` operation associated with `GetTokensResponse`.
#[derive(Debug, Clone)]
pub struct GetTokensResponseGetBuilder {
    param_q: Option<String>,
    param_type: Option<String>,
}

impl GetTokensResponseGetBuilder {
    #[inline]
    pub fn q(mut self, value: impl Into<String>) -> Self {
        self.param_q = Some(value.into());
        self
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> Self {
        self.param_type = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetTokensResponseGetBuilder {
    type Output = GetTokensResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/tokens".into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .header(http::header::ACCEPT.as_str(), "application/yaml")
        .query(&[
            ("q", self.param_q.as_ref().map(std::string::ToString::to_string)),
            ("type", self.param_type.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

