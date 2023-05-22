#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetSearchResponse<Any> {
    pub items: Vec<Any>,
    pub next_page_params: crate::get_search_response::GetSearchResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetSearchResponseNextPageParams {}

impl<Any: Default> GetSearchResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetSearchResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams, Any> {
        GetSearchResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn search() -> GetSearchResponseGetBuilder {
        GetSearchResponseGetBuilder {
            param_q: None,
        }
    }
}

impl<Any> Into<GetSearchResponse<Any>> for GetSearchResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists, Any> {
    fn into(self) -> GetSearchResponse<Any> {
        self.body
    }
}

/// Builder for [`GetSearchResponse`](./struct.GetSearchResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetSearchResponseBuilder<Items, NextPageParams, Any> {
    body: self::GetSearchResponse<Any>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams, Any> GetSearchResponseBuilder<Items, NextPageParams, Any> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = impl Into<Any>>) -> GetSearchResponseBuilder<crate::generics::ItemsExists, NextPageParams, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_search_response::GetSearchResponseNextPageParams) -> GetSearchResponseBuilder<Items, crate::generics::NextPageParamsExists, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetSearchResponse::search`](./struct.GetSearchResponse.html#method.search) method for a `GET` operation associated with `GetSearchResponse`.
#[derive(Debug, Clone)]
pub struct GetSearchResponseGetBuilder {
    param_q: Option<String>,
}

impl GetSearchResponseGetBuilder {
    #[inline]
    pub fn q(mut self, value: impl Into<String>) -> Self {
        self.param_q = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetSearchResponseGetBuilder {
    type Output = GetSearchResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/search".into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .header(http::header::ACCEPT.as_str(), "application/yaml")
        .query(&[
            ("q", self.param_q.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

