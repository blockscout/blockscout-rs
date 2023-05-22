#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetStatsChartsMarketResponse<Any> {
    pub available_supply: String,
    pub chart_data: Vec<Any>,
}

impl<Any: Default> GetStatsChartsMarketResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetStatsChartsMarketResponseBuilder<crate::generics::MissingAvailableSupply, crate::generics::MissingChartData, Any> {
        GetStatsChartsMarketResponseBuilder {
            body: Default::default(),
            _available_supply: core::marker::PhantomData,
            _chart_data: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_market_chart() -> GetStatsChartsMarketResponseGetBuilder {
        GetStatsChartsMarketResponseGetBuilder
    }
}

impl<Any> Into<GetStatsChartsMarketResponse<Any>> for GetStatsChartsMarketResponseBuilder<crate::generics::AvailableSupplyExists, crate::generics::ChartDataExists, Any> {
    fn into(self) -> GetStatsChartsMarketResponse<Any> {
        self.body
    }
}

/// Builder for [`GetStatsChartsMarketResponse`](./struct.GetStatsChartsMarketResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetStatsChartsMarketResponseBuilder<AvailableSupply, ChartData, Any> {
    body: self::GetStatsChartsMarketResponse<Any>,
    _available_supply: core::marker::PhantomData<AvailableSupply>,
    _chart_data: core::marker::PhantomData<ChartData>,
}

impl<AvailableSupply, ChartData, Any> GetStatsChartsMarketResponseBuilder<AvailableSupply, ChartData, Any> {
    #[inline]
    pub fn available_supply(mut self, value: impl Into<String>) -> GetStatsChartsMarketResponseBuilder<crate::generics::AvailableSupplyExists, ChartData, Any> {
        self.body.available_supply = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn chart_data(mut self, value: impl Iterator<Item = impl Into<Any>>) -> GetStatsChartsMarketResponseBuilder<AvailableSupply, crate::generics::ChartDataExists, Any> {
        self.body.chart_data = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetStatsChartsMarketResponse::get_market_chart`](./struct.GetStatsChartsMarketResponse.html#method.get_market_chart) method for a `GET` operation associated with `GetStatsChartsMarketResponse`.
#[derive(Debug, Clone)]
pub struct GetStatsChartsMarketResponseGetBuilder;


impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetStatsChartsMarketResponseGetBuilder {
    type Output = GetStatsChartsMarketResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/stats/charts/market".into()
    }
}
