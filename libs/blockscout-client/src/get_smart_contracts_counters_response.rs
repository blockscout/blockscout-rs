#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetSmartContractsCountersResponse {
    pub new_smart_contracts_24h: String,
    pub new_verified_smart_contracts_24h: String,
    pub smart_contracts: String,
    pub verified_smart_contracts: String,
}

impl GetSmartContractsCountersResponse {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetSmartContractsCountersResponseBuilder<crate::generics::MissingNewSmartContracts24h, crate::generics::MissingNewVerifiedSmartContracts24h, crate::generics::MissingSmartContracts, crate::generics::MissingVerifiedSmartContracts> {
        GetSmartContractsCountersResponseBuilder {
            body: Default::default(),
            _new_smart_contracts_24h: core::marker::PhantomData,
            _new_verified_smart_contracts_24h: core::marker::PhantomData,
            _smart_contracts: core::marker::PhantomData,
            _verified_smart_contracts: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_smart_contracts_counters() -> GetSmartContractsCountersResponseGetBuilder {
        GetSmartContractsCountersResponseGetBuilder
    }
}

impl Into<GetSmartContractsCountersResponse> for GetSmartContractsCountersResponseBuilder<crate::generics::NewSmartContracts24hExists, crate::generics::NewVerifiedSmartContracts24hExists, crate::generics::SmartContractsExists, crate::generics::VerifiedSmartContractsExists> {
    fn into(self) -> GetSmartContractsCountersResponse {
        self.body
    }
}

/// Builder for [`GetSmartContractsCountersResponse`](./struct.GetSmartContractsCountersResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetSmartContractsCountersResponseBuilder<NewSmartContracts24h, NewVerifiedSmartContracts24h, SmartContracts, VerifiedSmartContracts> {
    body: self::GetSmartContractsCountersResponse,
    _new_smart_contracts_24h: core::marker::PhantomData<NewSmartContracts24h>,
    _new_verified_smart_contracts_24h: core::marker::PhantomData<NewVerifiedSmartContracts24h>,
    _smart_contracts: core::marker::PhantomData<SmartContracts>,
    _verified_smart_contracts: core::marker::PhantomData<VerifiedSmartContracts>,
}

impl<NewSmartContracts24h, NewVerifiedSmartContracts24h, SmartContracts, VerifiedSmartContracts> GetSmartContractsCountersResponseBuilder<NewSmartContracts24h, NewVerifiedSmartContracts24h, SmartContracts, VerifiedSmartContracts> {
    #[inline]
    pub fn new_smart_contracts_24h(mut self, value: impl Into<String>) -> GetSmartContractsCountersResponseBuilder<crate::generics::NewSmartContracts24hExists, NewVerifiedSmartContracts24h, SmartContracts, VerifiedSmartContracts> {
        self.body.new_smart_contracts_24h = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn new_verified_smart_contracts_24h(mut self, value: impl Into<String>) -> GetSmartContractsCountersResponseBuilder<NewSmartContracts24h, crate::generics::NewVerifiedSmartContracts24hExists, SmartContracts, VerifiedSmartContracts> {
        self.body.new_verified_smart_contracts_24h = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn smart_contracts(mut self, value: impl Into<String>) -> GetSmartContractsCountersResponseBuilder<NewSmartContracts24h, NewVerifiedSmartContracts24h, crate::generics::SmartContractsExists, VerifiedSmartContracts> {
        self.body.smart_contracts = value.into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn verified_smart_contracts(mut self, value: impl Into<String>) -> GetSmartContractsCountersResponseBuilder<NewSmartContracts24h, NewVerifiedSmartContracts24h, SmartContracts, crate::generics::VerifiedSmartContractsExists> {
        self.body.verified_smart_contracts = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetSmartContractsCountersResponse::get_smart_contracts_counters`](./struct.GetSmartContractsCountersResponse.html#method.get_smart_contracts_counters) method for a `GET` operation associated with `GetSmartContractsCountersResponse`.
#[derive(Debug, Clone)]
pub struct GetSmartContractsCountersResponseGetBuilder;


impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetSmartContractsCountersResponseGetBuilder {
    type Output = GetSmartContractsCountersResponse;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/smart-contracts/counters".into()
    }
}
