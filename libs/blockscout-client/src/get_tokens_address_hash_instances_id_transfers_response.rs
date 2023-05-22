#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTokensAddressHashInstancesIdTransfersResponse<Any> {
    pub items: Vec<crate::token_transfer::TokenTransfer<Any>>,
    pub next_page_params: crate::get_tokens_address_hash_instances_id_transfers_response::GetTokensAddressHashInstancesIdTransfersResponseNextPageParams,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTokensAddressHashInstancesIdTransfersResponseNextPageParams {}

impl<Any: Default> GetTokensAddressHashInstancesIdTransfersResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetTokensAddressHashInstancesIdTransfersResponseBuilder<crate::generics::MissingItems, crate::generics::MissingNextPageParams, Any> {
        GetTokensAddressHashInstancesIdTransfersResponseBuilder {
            body: Default::default(),
            _items: core::marker::PhantomData,
            _next_page_params: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_nft_instance_transfers() -> GetTokensAddressHashInstancesIdTransfersResponseGetBuilder<crate::generics::MissingAddressHash, crate::generics::MissingId> {
        GetTokensAddressHashInstancesIdTransfersResponseGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
            _param_id: core::marker::PhantomData,
        }
    }
}

impl<Any> Into<GetTokensAddressHashInstancesIdTransfersResponse<Any>> for GetTokensAddressHashInstancesIdTransfersResponseBuilder<crate::generics::ItemsExists, crate::generics::NextPageParamsExists, Any> {
    fn into(self) -> GetTokensAddressHashInstancesIdTransfersResponse<Any> {
        self.body
    }
}

/// Builder for [`GetTokensAddressHashInstancesIdTransfersResponse`](./struct.GetTokensAddressHashInstancesIdTransfersResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetTokensAddressHashInstancesIdTransfersResponseBuilder<Items, NextPageParams, Any> {
    body: self::GetTokensAddressHashInstancesIdTransfersResponse<Any>,
    _items: core::marker::PhantomData<Items>,
    _next_page_params: core::marker::PhantomData<NextPageParams>,
}

impl<Items, NextPageParams, Any> GetTokensAddressHashInstancesIdTransfersResponseBuilder<Items, NextPageParams, Any> {
    #[inline]
    pub fn items(mut self, value: impl Iterator<Item = crate::token_transfer::TokenTransferBuilder<crate::generics::FromExists, crate::generics::ToExists, crate::generics::TokenExists, crate::generics::TotalExists, crate::generics::TxHashExists, crate::generics::TypeExists, Any>>) -> GetTokensAddressHashInstancesIdTransfersResponseBuilder<crate::generics::ItemsExists, NextPageParams, Any> {
        self.body.items = value.map(|value| value.into()).collect::<Vec<_>>().into();
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn next_page_params(mut self, value: crate::get_tokens_address_hash_instances_id_transfers_response::GetTokensAddressHashInstancesIdTransfersResponseNextPageParams) -> GetTokensAddressHashInstancesIdTransfersResponseBuilder<Items, crate::generics::NextPageParamsExists, Any> {
        self.body.next_page_params = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetTokensAddressHashInstancesIdTransfersResponse::get_nft_instance_transfers`](./struct.GetTokensAddressHashInstancesIdTransfersResponse.html#method.get_nft_instance_transfers) method for a `GET` operation associated with `GetTokensAddressHashInstancesIdTransfersResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetTokensAddressHashInstancesIdTransfersResponseGetBuilder<AddressHash, Id> {
    inner: GetTokensAddressHashInstancesIdTransfersResponseGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
    _param_id: core::marker::PhantomData<Id>,
}

#[derive(Debug, Default, Clone)]
struct GetTokensAddressHashInstancesIdTransfersResponseGetBuilderContainer {
    param_address_hash: Option<String>,
    param_id: Option<i64>,
}

impl<AddressHash, Id> GetTokensAddressHashInstancesIdTransfersResponseGetBuilder<AddressHash, Id> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> GetTokensAddressHashInstancesIdTransfersResponseGetBuilder<crate::generics::AddressHashExists, Id> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }

    /// integer id
    #[inline]
    pub fn id(mut self, value: impl Into<i64>) -> GetTokensAddressHashInstancesIdTransfersResponseGetBuilder<AddressHash, crate::generics::IdExists> {
        self.inner.param_id = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetTokensAddressHashInstancesIdTransfersResponseGetBuilder<crate::generics::AddressHashExists, crate::generics::IdExists> {
    type Output = GetTokensAddressHashInstancesIdTransfersResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/tokens/{address_hash}/instances/{id}/transfers", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?"), id=self.inner.param_id.as_ref().expect("missing parameter id?")).into()
    }
}

