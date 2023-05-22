#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GetTokensAddressHashInstancesIdTransfersCountResponse<Any> {
    pub transfers_count: i64,
}

impl<Any: Default> GetTokensAddressHashInstancesIdTransfersCountResponse<Any> {
    /// Create a builder for this object.
    #[inline]
    pub fn builder() -> GetTokensAddressHashInstancesIdTransfersCountResponseBuilder<crate::generics::MissingTransfersCount, Any> {
        GetTokensAddressHashInstancesIdTransfersCountResponseBuilder {
            body: Default::default(),
            _transfers_count: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_nft_instance_transfers_count() -> GetTokensAddressHashInstancesIdTransfersCountResponseGetBuilder<crate::generics::MissingAddressHash, crate::generics::MissingId> {
        GetTokensAddressHashInstancesIdTransfersCountResponseGetBuilder {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
            _param_id: core::marker::PhantomData,
        }
    }
}

impl<Any> Into<GetTokensAddressHashInstancesIdTransfersCountResponse<Any>> for GetTokensAddressHashInstancesIdTransfersCountResponseBuilder<crate::generics::TransfersCountExists, Any> {
    fn into(self) -> GetTokensAddressHashInstancesIdTransfersCountResponse<Any> {
        self.body
    }
}

/// Builder for [`GetTokensAddressHashInstancesIdTransfersCountResponse`](./struct.GetTokensAddressHashInstancesIdTransfersCountResponse.html) object.
#[derive(Debug, Clone)]
pub struct GetTokensAddressHashInstancesIdTransfersCountResponseBuilder<TransfersCount, Any> {
    body: self::GetTokensAddressHashInstancesIdTransfersCountResponse<Any>,
    _transfers_count: core::marker::PhantomData<TransfersCount>,
}

impl<TransfersCount, Any> GetTokensAddressHashInstancesIdTransfersCountResponseBuilder<TransfersCount, Any> {
    #[inline]
    pub fn transfers_count(mut self, value: impl Into<i64<Any>>) -> GetTokensAddressHashInstancesIdTransfersCountResponseBuilder<crate::generics::TransfersCountExists, Any> {
        self.body.transfers_count = value.into();
        unsafe { std::mem::transmute(self) }
    }
}

/// Builder created by [`GetTokensAddressHashInstancesIdTransfersCountResponse::get_nft_instance_transfers_count`](./struct.GetTokensAddressHashInstancesIdTransfersCountResponse.html#method.get_nft_instance_transfers_count) method for a `GET` operation associated with `GetTokensAddressHashInstancesIdTransfersCountResponse`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct GetTokensAddressHashInstancesIdTransfersCountResponseGetBuilder<AddressHash, Id> {
    inner: GetTokensAddressHashInstancesIdTransfersCountResponseGetBuilderContainer,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
    _param_id: core::marker::PhantomData<Id>,
}

#[derive(Debug, Default, Clone)]
struct GetTokensAddressHashInstancesIdTransfersCountResponseGetBuilderContainer {
    param_address_hash: Option<String>,
    param_id: Option<i64>,
}

impl<AddressHash, Id> GetTokensAddressHashInstancesIdTransfersCountResponseGetBuilder<AddressHash, Id> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> GetTokensAddressHashInstancesIdTransfersCountResponseGetBuilder<crate::generics::AddressHashExists, Id> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }

    /// integer id
    #[inline]
    pub fn id(mut self, value: impl Into<i64>) -> GetTokensAddressHashInstancesIdTransfersCountResponseGetBuilder<AddressHash, crate::generics::IdExists> {
        self.inner.param_id = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for GetTokensAddressHashInstancesIdTransfersCountResponseGetBuilder<crate::generics::AddressHashExists, crate::generics::IdExists> {
    type Output = GetTokensAddressHashInstancesIdTransfersCountResponse<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/tokens/{address_hash}/instances/{id}/transfers-count", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?"), id=self.inner.param_id.as_ref().expect("missing parameter id?")).into()
    }
}
