#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetSmartContractsResponse<Any> {
    pub items: Vec<Any>,
    pub next_page_params: crate::get_smart_contracts_response::GetSmartContractsResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetSmartContractsResponseNextPageParams {}

impl<Any: Default> GetSmartContractsResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetSmartContractsResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams, Any> {
        GetSmartContractsResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_smart_contracts() -> GetSmartContractsResponseGetBuilder {
        GetSmartContractsResponseGetBuilder {
            param_q: None,
            param_filter: None,
        }
    }
}

impl<Any> Into<GetSmartContractsResponse<Any>> for GetSmartContractsResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists, Any> {
    fn into(self) -> GetSmartContractsResponse<Any> {
        self.body
    }
}

/// Builder for [`GetSmartContractsResponse`](./struct.GetSmartContractsResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetSmartContractsResponseBuilder<Items, NextPageParams, Any> {
    body: self::GetSmartContractsResponse<Any>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams, Any> GetSmartContractsResponseBuilder<Items, NextPageParams, Any> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = impl Into<Any>>) -> GetSmartContractsResponseBuilder<crate::generics::ItemsExists, NextPageParams, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_smart_contracts_response::GetSmartContractsResponseNextPageParams) -> GetSmartContractsResponseBuilder<Items, crate::generics::NextPageParamsExists, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetSmartContractsResponse::get_smart_contracts`](./struct.GetSmartContractsResponse.html#method.get_smart_contracts) method for a `GET` operation associated with `GetSmartContractsResponse`.
#[derive(Debug, Clone)]
pub struct GetSmartContractsResponseGetBuilder {
    param_q: Option<String>,
    param_filter: Option<String>,
}

impl GetSmartContractsResponseGetBuilder {
    #[inline]
    pub fn q(mut self, value: impl Into<String>) -> Self {
        self.param_q = Some(value.into());
        self
    }

    #[inline]
    pub fn filter(mut self, value: impl Into<String>) -> Self {
        self.param_filter = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetSmartContractsResponseGetBuilder {
    type Output = GetSmartContractsResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/smart-contracts".into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .header(http::header::ACCEPT.as_str(), "application/yaml")
        .query(&[
            ("q", self.param_q.as_ref().map(std::string::ToString::to_string)),
            ("filter", self.param_filter.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

