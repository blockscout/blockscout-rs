#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTransactionsResponse<Any> {
    pub items: Vec<crate::transaction::Transaction<Any>>,
    pub next_page_params: crate::get_transactions_response::GetTransactionsResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTransactionsResponseNextPageParams {}

impl<Any: Default> GetTransactionsResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetTransactionsResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams, Any> {
        GetTransactionsResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_txs() -> GetTransactionsResponseGetBuilder {
        GetTransactionsResponseGetBuilder {
            param_filter: None,
            param_type: None,
            param_method: None,
        }
    }
}

impl<Any> Into<GetTransactionsResponse<Any>> for GetTransactionsResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists, Any> {
    fn into(self) -> GetTransactionsResponse<Any> {
        self.body
    }
}

/// Builder for [`GetTransactionsResponse`](./struct.GetTransactionsResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetTransactionsResponseBuilder<Items, NextPageParams, Any> {
    body: self::GetTransactionsResponse<Any>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams, Any> GetTransactionsResponseBuilder<Items, NextPageParams, Any> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::transaction::TransactionBuilder<crate::generics::ConfirmationDurationExists, crate::generics::ConfirmationsExists, crate::generics::CreatedContractExists, crate::generics::FeeExists, crate::generics::FromExists, crate::generics::GasLimitExists, crate::generics::GasPriceExists, crate::generics::HashExists, crate::generics::NonceExists, crate::generics::RawInputExists, crate::generics::ResultExists, crate::generics::ToExists, crate::generics::TxTypesExists, crate::generics::ValueExists, Any>>) -> GetTransactionsResponseBuilder<crate::generics::ItemsExists, NextPageParams, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_transactions_response::GetTransactionsResponseNextPageParams) -> GetTransactionsResponseBuilder<Items, crate::generics::NextPageParamsExists, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetTransactionsResponse::get_txs`](./struct.GetTransactionsResponse.html#method.get_txs) method for a `GET` operation associated with `GetTransactionsResponse`.
#[derive(Debug, Clone)]
pub struct GetTransactionsResponseGetBuilder {
    param_filter: Option<String>,
    param_type: Option<String>,
    param_method: Option<String>,
}

impl GetTransactionsResponseGetBuilder {
    #[inline]
    pub fn filter(mut self, value: impl Into<String>) -> Self {
        self.param_filter = Some(value.into());
        self
    }

    #[inline]
    pub fn type_(mut self, value: impl Into<String>) -> Self {
        self.param_type = Some(value.into());
        self
    }

    #[inline]
    pub fn method(mut self, value: impl Into<String>) -> Self {
        self.param_method = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetTransactionsResponseGetBuilder {
    type Output = GetTransactionsResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/transactions".into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .header(http::header::ACCEPT.as_str(), "application/yaml")
        .query(&[
            ("filter", self.param_filter.as_ref().map(std::string::ToString::to_string)),
            ("type", self.param_type.as_ref().map(std::string::ToString::to_string)),
            ("method", self.param_method.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

