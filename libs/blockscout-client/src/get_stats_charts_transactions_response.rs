#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetStatsChartsTransactionsResponse<Any> {
    pub chart_data: Vec<Any>,
}

impl<Any: Default> GetStatsChartsTransactionsResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetStatsChartsTransactionsResponseBuilder<crate::generics::MissingChartData, Any> {
        GetStatsChartsTransactionsResponseBuilder {
            body: Default::default(),
            _chart_data: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_txs_chart() -> GetStatsChartsTransactionsResponseGetBuilder {
        GetStatsChartsTransactionsResponseGetBuilder
    }
}

impl<Any> Into<GetStatsChartsTransactionsResponse<Any>> for GetStatsChartsTransactionsResponseBuilder<crate::generics::ChartDataExists, Any> {
    fn into(self) -> GetStatsChartsTransactionsResponse<Any> {
        self.body
    }
}

/// Builder for [`GetStatsChartsTransactionsResponse`](./struct.GetStatsChartsTransactionsResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetStatsChartsTransactionsResponseBuilder<ChartData, Any> {
    body: self::GetStatsChartsTransactionsResponse<Any>,
    _chart_data: core::marker::PhantomData<ChartData>,
}

impl<ChartData, Any> GetStatsChartsTransactionsResponseBuilder<ChartData, Any> {
    #[inline]
    pub fn chart_data(mut self, value: impl Iterator<Item = impl Into<Any>>) -> GetStatsChartsTransactionsResponseBuilder<crate::generics::ChartDataExists, Any> {
        self.body.chart_data = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetStatsChartsTransactionsResponse::get_txs_chart`](./struct.GetStatsChartsTransactionsResponse.html#method.get_txs_chart) method for a `GET` operation associated with `GetStatsChartsTransactionsResponse`.
#[derive(Debug, Clone)]
pub struct GetStatsChartsTransactionsResponseGetBuilder;


impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetStatsChartsTransactionsResponseGetBuilder {
    type Output = GetStatsChartsTransactionsResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/stats/charts/transactions".into()
    }
}
