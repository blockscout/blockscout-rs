#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetBlocksResponse {
    pub items: Vec<crate::block::Block>,
    pub next_page_params: crate::get_blocks_response::GetBlocksResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetBlocksResponseNextPageParams {}

impl GetBlocksResponse {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetBlocksResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams> {
        GetBlocksResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_blocks() -> GetBlocksResponseGetBuilder {
        GetBlocksResponseGetBuilder {
            param_type: None,
        }
    }
}

impl Into<GetBlocksResponse> for GetBlocksResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists> {
    fn into(self) -> GetBlocksResponse {
        self.body
    }
}

/// Builder for [`GetBlocksResponse`](./struct.GetBlocksResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetBlocksResponseBuilder<Items, NextPageParams> {
    body: self::GetBlocksResponse,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams> GetBlocksResponseBuilder<Items, NextPageParams> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::block::BlockBuilder<crate::generics::DifficultyExists, crate::generics::GasLimitExists, crate::generics::GasUsedExists, crate::generics::HashExists, crate::generics::HeightExists, crate::generics::MinerExists, crate::generics::NonceExists, crate::generics::ParentHashExists, crate::generics::SizeExists, crate::generics::TimestampExists, crate::generics::TotalDifficultyExists, crate::generics::TxCountExists>>) -> GetBlocksResponseBuilder<crate::generics::ItemsExists, NextPageParams> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_blocks_response::GetBlocksResponseNextPageParams) -> GetBlocksResponseBuilder<Items, crate::generics::NextPageParamsExists> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetBlocksResponse::get_blocks`](./struct.GetBlocksResponse.html#method.get_blocks) method for a `GET` operation associated with `GetBlocksResponse`.
#[derive(Debug, Clone)]
pub struct GetBlocksResponseGetBuilder {
    param_type: Option<String>,
}

impl GetBlocksResponseGetBuilder {
    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> Self {
        self.param_type = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetBlocksResponseGetBuilder {
    type Output = GetBlocksResponse;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/blocks".into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .query(&[
            ("type", self.param_type.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

