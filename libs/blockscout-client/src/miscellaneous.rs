
/// Namespace for operations that cannot be added to any other modules.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Miscellaneous {}

impl Miscellaneous {
    #[inline]
    pub fn get_indexing_status() -> MiscellaneousGetBuilder {
        MiscellaneousGetBuilder
    }

    #[inline]
    pub fn search_redirect() -> MiscellaneousGetBuilder1 {
        MiscellaneousGetBuilder1 {
            param_q: None,
        }
    }

    #[inline]
    pub fn get_smart_contract() -> MiscellaneousGetBuilder2<crate::generics::MissingAddressHash> {
        MiscellaneousGetBuilder2 {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_read_methods() -> MiscellaneousGetBuilder3<crate::generics::MissingAddressHash> {
        MiscellaneousGetBuilder3 {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_read_methods_proxy() -> MiscellaneousGetBuilder4<crate::generics::MissingAddressHash> {
        MiscellaneousGetBuilder4 {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_write_methods() -> MiscellaneousGetBuilder5<crate::generics::MissingAddressHash> {
        MiscellaneousGetBuilder5 {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_write_methods_proxy() -> MiscellaneousGetBuilder6<crate::generics::MissingAddressHash> {
        MiscellaneousGetBuilder6 {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn query_read_method() -> MiscellaneousPostBuilder7<crate::generics::MissingAddressHash> {
        MiscellaneousPostBuilder7 {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_stats() -> MiscellaneousGetBuilder8 {
        MiscellaneousGetBuilder8
    }

    #[inline]
    pub fn get_token() -> MiscellaneousGetBuilder9<crate::generics::MissingAddressHash> {
        MiscellaneousGetBuilder9 {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_token_counters() -> MiscellaneousGetBuilder10<crate::generics::MissingAddressHash> {
        MiscellaneousGetBuilder10 {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_nft_instance() -> MiscellaneousGetBuilder11<crate::generics::MissingAddressHash, crate::generics::MissingId> {
        MiscellaneousGetBuilder11 {
            inner: Default::default(),
            _param_address_hash: core::marker::PhantomData,
            _param_id: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn get_state_changes() -> MiscellaneousGetBuilder12<crate::generics::MissingTransactionHash> {
        MiscellaneousGetBuilder12 {
            inner: Default::default(),
            _param_transaction_hash: core::marker::PhantomData,
        }
    }
}

/// Builder created by [`Miscellaneous::get_indexing_status`](./struct.Miscellaneous.html#method.get_indexing_status) method for a `GET` operation associated with `Miscellaneous`.
#[derive(Debug, Clone)]
pub struct MiscellaneousGetBuilder;


impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousGetBuilder {
    type Output = Any<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/main-page/indexing-status".into()
    }
}

/// Builder created by [`Miscellaneous::search_redirect`](./struct.Miscellaneous.html#method.search_redirect) method for a `GET` operation associated with `Miscellaneous`.
#[derive(Debug, Clone)]
pub struct MiscellaneousGetBuilder1 {
    param_q: Option<String>,
}

impl MiscellaneousGetBuilder1 {
    #[inline]
    pub fn q(mut self, value: impl Into<String>) -> Self {
        self.param_q = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousGetBuilder1 {
    type Output = Any<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/search/check-redirect".into()
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

/// Builder created by [`Miscellaneous::get_smart_contract`](./struct.Miscellaneous.html#method.get_smart_contract) method for a `GET` operation associated with `Miscellaneous`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct MiscellaneousGetBuilder2<AddressHash> {
    inner: MiscellaneousGetBuilder2Container,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct MiscellaneousGetBuilder2Container {
    param_address_hash: Option<String>,
}

impl<AddressHash> MiscellaneousGetBuilder2<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> MiscellaneousGetBuilder2<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousGetBuilder2<crate::generics::AddressHashExists> {
    type Output = Any<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/smart-contracts/{address_hash}", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}

/// Builder created by [`Miscellaneous::get_read_methods`](./struct.Miscellaneous.html#method.get_read_methods) method for a `GET` operation associated with `Miscellaneous`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct MiscellaneousGetBuilder3<AddressHash> {
    inner: MiscellaneousGetBuilder3Container,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct MiscellaneousGetBuilder3Container {
    param_address_hash: Option<String>,
    param_is_custom_abi: Option<String>,
    param_from: Option<String>,
}

impl<AddressHash> MiscellaneousGetBuilder3<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> MiscellaneousGetBuilder3<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn is_custom_abi(mut self, value: impl Into<String>) -> Self {
        self.inner.param_is_custom_abi = Some(value.into());
        self
    }

    #[inline]
    pub fn from(mut self, value: impl Into<String>) -> Self {
        self.inner.param_from = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousGetBuilder3<crate::generics::AddressHashExists> {
    type Output = Vec<Any><serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/smart-contracts/{address_hash}/methods-read", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .header(http::header::ACCEPT.as_str(), "application/yaml")
        .query(&[
            ("is_custom_abi", self.inner.param_is_custom_abi.as_ref().map(std::string::ToString::to_string)),
            ("from", self.inner.param_from.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

/// Builder created by [`Miscellaneous::get_read_methods_proxy`](./struct.Miscellaneous.html#method.get_read_methods_proxy) method for a `GET` operation associated with `Miscellaneous`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct MiscellaneousGetBuilder4<AddressHash> {
    inner: MiscellaneousGetBuilder4Container,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct MiscellaneousGetBuilder4Container {
    param_address_hash: Option<String>,
    param_is_custom_abi: Option<String>,
    param_from: Option<String>,
}

impl<AddressHash> MiscellaneousGetBuilder4<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> MiscellaneousGetBuilder4<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn is_custom_abi(mut self, value: impl Into<String>) -> Self {
        self.inner.param_is_custom_abi = Some(value.into());
        self
    }

    #[inline]
    pub fn from(mut self, value: impl Into<String>) -> Self {
        self.inner.param_from = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousGetBuilder4<crate::generics::AddressHashExists> {
    type Output = Vec<Any><serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/smart-contracts/{address_hash}/methods-read-proxy", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .header(http::header::ACCEPT.as_str(), "application/yaml")
        .query(&[
            ("is_custom_abi", self.inner.param_is_custom_abi.as_ref().map(std::string::ToString::to_string)),
            ("from", self.inner.param_from.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

/// Builder created by [`Miscellaneous::get_write_methods`](./struct.Miscellaneous.html#method.get_write_methods) method for a `GET` operation associated with `Miscellaneous`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct MiscellaneousGetBuilder5<AddressHash> {
    inner: MiscellaneousGetBuilder5Container,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct MiscellaneousGetBuilder5Container {
    param_address_hash: Option<String>,
    param_is_custom_abi: Option<String>,
}

impl<AddressHash> MiscellaneousGetBuilder5<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> MiscellaneousGetBuilder5<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn is_custom_abi(mut self, value: impl Into<String>) -> Self {
        self.inner.param_is_custom_abi = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousGetBuilder5<crate::generics::AddressHashExists> {
    type Output = Vec<Any><serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/smart-contracts/{address_hash}/methods-write", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .header(http::header::ACCEPT.as_str(), "application/yaml")
        .query(&[
            ("is_custom_abi", self.inner.param_is_custom_abi.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

/// Builder created by [`Miscellaneous::get_write_methods_proxy`](./struct.Miscellaneous.html#method.get_write_methods_proxy) method for a `GET` operation associated with `Miscellaneous`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct MiscellaneousGetBuilder6<AddressHash> {
    inner: MiscellaneousGetBuilder6Container,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct MiscellaneousGetBuilder6Container {
    param_address_hash: Option<String>,
    param_is_custom_abi: Option<String>,
}

impl<AddressHash> MiscellaneousGetBuilder6<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> MiscellaneousGetBuilder6<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }

    #[inline]
    pub fn is_custom_abi(mut self, value: impl Into<String>) -> Self {
        self.inner.param_is_custom_abi = Some(value.into());
        self
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousGetBuilder6<crate::generics::AddressHashExists> {
    type Output = Vec<Any><serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/smart-contracts/{address_hash}/methods-write-proxy", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }

    fn modify(&self, req: Client::Request) -> Result<Client::Request, crate::client::ApiError<Client::Response>> {
        use crate::client::Request;
        Ok(req
        .header(http::header::ACCEPT.as_str(), "application/yaml")
        .query(&[
            ("is_custom_abi", self.inner.param_is_custom_abi.as_ref().map(std::string::ToString::to_string))
        ]))
    }
}

/// Builder created by [`Miscellaneous::query_read_method`](./struct.Miscellaneous.html#method.query_read_method) method for a `POST` operation associated with `Miscellaneous`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct MiscellaneousPostBuilder7<AddressHash> {
    inner: MiscellaneousPostBuilder7Container,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct MiscellaneousPostBuilder7Container {
    param_address_hash: Option<String>,
}

impl<AddressHash> MiscellaneousPostBuilder7<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> MiscellaneousPostBuilder7<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousPostBuilder7<crate::generics::AddressHashExists> {
    type Output = Vec<Any><serde_yaml::Value>;

    const METHOD: http::Method = http::Method::POST;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/smart-contracts/{address_hash}/query-read-method", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}

/// Builder created by [`Miscellaneous::get_stats`](./struct.Miscellaneous.html#method.get_stats) method for a `GET` operation associated with `Miscellaneous`.
#[derive(Debug, Clone)]
pub struct MiscellaneousGetBuilder8;


impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousGetBuilder8 {
    type Output = Any<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        "/stats".into()
    }
}

/// Builder created by [`Miscellaneous::get_token`](./struct.Miscellaneous.html#method.get_token) method for a `GET` operation associated with `Miscellaneous`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct MiscellaneousGetBuilder9<AddressHash> {
    inner: MiscellaneousGetBuilder9Container,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct MiscellaneousGetBuilder9Container {
    param_address_hash: Option<String>,
}

impl<AddressHash> MiscellaneousGetBuilder9<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> MiscellaneousGetBuilder9<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousGetBuilder9<crate::generics::AddressHashExists> {
    type Output = Any<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/tokens/{address_hash}", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}

/// Builder created by [`Miscellaneous::get_token_counters`](./struct.Miscellaneous.html#method.get_token_counters) method for a `GET` operation associated with `Miscellaneous`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct MiscellaneousGetBuilder10<AddressHash> {
    inner: MiscellaneousGetBuilder10Container,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
}

#[derive(Debug, Default, Clone)]
struct MiscellaneousGetBuilder10Container {
    param_address_hash: Option<String>,
}

impl<AddressHash> MiscellaneousGetBuilder10<AddressHash> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> MiscellaneousGetBuilder10<crate::generics::AddressHashExists> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousGetBuilder10<crate::generics::AddressHashExists> {
    type Output = Any<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/tokens/{address_hash}/counters", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?")).into()
    }
}

/// Builder created by [`Miscellaneous::get_nft_instance`](./struct.Miscellaneous.html#method.get_nft_instance) method for a `GET` operation associated with `Miscellaneous`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct MiscellaneousGetBuilder11<AddressHash, Id> {
    inner: MiscellaneousGetBuilder11Container,
    _param_address_hash: core::marker::PhantomData<AddressHash>,
    _param_id: core::marker::PhantomData<Id>,
}

#[derive(Debug, Default, Clone)]
struct MiscellaneousGetBuilder11Container {
    param_address_hash: Option<String>,
    param_id: Option<i64>,
}

impl<AddressHash, Id> MiscellaneousGetBuilder11<AddressHash, Id> {
    /// Address hash
    #[inline]
    pub fn address_hash(mut self, value: impl Into<String>) -> MiscellaneousGetBuilder11<crate::generics::AddressHashExists, Id> {
        self.inner.param_address_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }

    /// integer id
    #[inline]
    pub fn id(mut self, value: impl Into<i64>) -> MiscellaneousGetBuilder11<AddressHash, crate::generics::IdExists> {
        self.inner.param_id = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousGetBuilder11<crate::generics::AddressHashExists, crate::generics::IdExists> {
    type Output = Any<serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/tokens/{address_hash}/instances/{id}", address_hash=self.inner.param_address_hash.as_ref().expect("missing parameter address_hash?"), id=self.inner.param_id.as_ref().expect("missing parameter id?")).into()
    }
}

/// Builder created by [`Miscellaneous::get_state_changes`](./struct.Miscellaneous.html#method.get_state_changes) method for a `GET` operation associated with `Miscellaneous`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct MiscellaneousGetBuilder12<TransactionHash> {
    inner: MiscellaneousGetBuilder12Container,
    _param_transaction_hash: core::marker::PhantomData<TransactionHash>,
}

#[derive(Debug, Default, Clone)]
struct MiscellaneousGetBuilder12Container {
    param_transaction_hash: Option<String>,
}

impl<TransactionHash> MiscellaneousGetBuilder12<TransactionHash> {
    /// Transaction hash
    #[inline]
    pub fn transaction_hash(mut self, value: impl Into<String>) -> MiscellaneousGetBuilder12<crate::generics::TransactionHashExists> {
        self.inner.param_transaction_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for MiscellaneousGetBuilder12<crate::generics::TransactionHashExists> {
    type Output = Vec<Any><serde_yaml::Value>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/transactions/{transaction_hash}/state-changes", transaction_hash=self.inner.param_transaction_hash.as_ref().expect("missing parameter transaction_hash?")).into()
    }
}
