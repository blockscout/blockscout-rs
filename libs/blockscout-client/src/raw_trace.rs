#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RawTrace {}

impl RawTrace {
    #[inline]
    pub fn get_raw_trace() -> RawTraceGetBuilder<crate::generics::MissingTransactionHash> {
        RawTraceGetBuilder {
            inner: Default::default(),
            _param_transaction_hash: core::marker::PhantomData,
        }
    }
}

/// Builder created by [`RawTrace::get_raw_trace`](./struct.RawTrace.html#method.get_raw_trace) method for a `GET` operation associated with `RawTrace`.
#[repr(transparent)]
#[derive(Debug, Clone)]
pub struct RawTraceGetBuilder<TransactionHash> {
    inner: RawTraceGetBuilderContainer,
    _param_transaction_hash: core::marker::PhantomData<TransactionHash>,
}

#[derive(Debug, Default, Clone)]
struct RawTraceGetBuilderContainer {
    param_transaction_hash: Option<String>,
}

impl<TransactionHash> RawTraceGetBuilder<TransactionHash> {
    /// Transaction hash
    #[inline]
    pub fn transaction_hash(mut self, value: impl Into<String>) -> RawTraceGetBuilder<crate::generics::TransactionHashExists> {
        self.inner.param_transaction_hash = Some(value.into());
        unsafe { std::mem::transmute(self) }
    }
}

impl<Client: crate::client::ApiClient + Sync + 'static> crate::client::Sendable<Client> for RawTraceGetBuilder<crate::generics::TransactionHashExists> {
    type Output = Vec<RawTrace>;

    const METHOD: http::Method = http::Method::GET;

    fn rel_path(&self) -> std::borrow::Cow<'static, str> {
        format!("/transactions/{transaction_hash}/raw-trace", transaction_hash=self.inner.param_transaction_hash.as_ref().expect("missing parameter transaction_hash?")).into()
    }
}
