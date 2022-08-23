#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAbiRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub chain_id: u64,
}
/// todo!()
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAbiResponse {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSourcesRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub chain_id: u64,
}
/// todo!()
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSourcesResponse {}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifySingleRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub creation_tx_input: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub deployed_bytecode: ::prost::alloc::vec::Vec<u8>,
    /// as one instance of the service could be used for multiple chains
    #[prost(uint64, tag = "4")]
    pub chain_id: u64,
    #[prost(string, tag = "5")]
    pub compiler_version: ::prost::alloc::string::String,
    /// if None, `evm_version` is set to "default"
    #[prost(string, optional, tag = "6")]
    pub evm_version: ::core::option::Option<::prost::alloc::string::String>,
    /// if None, `optimizer.enabled` is set to be false
    #[prost(uint32, optional, tag = "7")]
    pub optimization_runs: ::core::option::Option<u32>,
    /// library_name -> library_address;
    #[prost(map = "string, string", tag = "8")]
    pub libraries:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    #[prost(string, tag = "9")]
    pub source_code: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifySingleResponse {
    #[prost(uint32, tag = "1")]
    pub status: u32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub result: ::core::option::Option<verify_single_response::Result>,
}
/// Nested message and enum types in `VerifySingleResponse`.
pub mod verify_single_response {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Result {
        #[prost(string, tag = "1")]
        pub name: ::prost::alloc::string::String,
        #[prost(string, tag = "2")]
        pub compiler_version: ::prost::alloc::string::String,
        #[prost(string, tag = "3")]
        pub evm_version: ::prost::alloc::string::String,
        #[prost(bool, optional, tag = "4")]
        pub optimization: ::core::option::Option<bool>,
        #[prost(uint32, optional, tag = "5")]
        pub optimization_runs: ::core::option::Option<u32>,
        #[prost(map = "string, string", tag = "6")]
        pub libraries: ::std::collections::HashMap<
            ::prost::alloc::string::String,
            ::prost::alloc::string::String,
        >,
        #[prost(string, tag = "7")]
        pub source_code: ::prost::alloc::string::String,
        #[prost(string, tag = "9")]
        pub abi: ::prost::alloc::string::String,
        #[prost(bytes = "vec", tag = "10")]
        pub constructor_arguments: ::prost::alloc::vec::Vec<u8>,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyMultiRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub creation_tx_input: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub deployed_bytecode: ::prost::alloc::vec::Vec<u8>,
    /// as one instance of the service could be used for multiple chains
    #[prost(uint64, tag = "4")]
    pub chain_id: u64,
    #[prost(string, tag = "5")]
    pub compiler_version: ::prost::alloc::string::String,
    /// if None, `evm_version` is set to "default"
    #[prost(string, optional, tag = "6")]
    pub evm_version: ::core::option::Option<::prost::alloc::string::String>,
    /// if None, `optimizer.enabled` is set to be false
    #[prost(uint32, optional, tag = "7")]
    pub optimization_runs: ::core::option::Option<u32>,
    /// library_name -> library_address;
    #[prost(map = "string, string", tag = "8")]
    pub libraries:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    #[prost(map = "string, string", tag = "9")]
    pub sources:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyMultiResponse {
    #[prost(uint32, tag = "1")]
    pub status: u32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub result: ::core::option::Option<verify_multi_response::Result>,
}
/// Nested message and enum types in `VerifyMultiResponse`.
pub mod verify_multi_response {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Result {
        #[prost(string, tag = "1")]
        pub file_path: ::prost::alloc::string::String,
        #[prost(string, tag = "2")]
        pub name: ::prost::alloc::string::String,
        #[prost(string, tag = "3")]
        pub compiler_version: ::prost::alloc::string::String,
        #[prost(string, tag = "4")]
        pub evm_version: ::prost::alloc::string::String,
        #[prost(bool, optional, tag = "5")]
        pub optimization: ::core::option::Option<bool>,
        #[prost(uint32, optional, tag = "6")]
        pub optimization_runs: ::core::option::Option<u32>,
        #[prost(map = "string, string", tag = "7")]
        pub libraries: ::std::collections::HashMap<
            ::prost::alloc::string::String,
            ::prost::alloc::string::String,
        >,
        #[prost(map = "string, string", tag = "8")]
        pub sources: ::std::collections::HashMap<
            ::prost::alloc::string::String,
            ::prost::alloc::string::String,
        >,
        #[prost(string, tag = "9")]
        pub abi: ::prost::alloc::string::String,
        #[prost(bytes = "vec", tag = "10")]
        pub constructor_arguments: ::prost::alloc::vec::Vec<u8>,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyStandardJsonRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub creation_tx_input: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub deployed_bytecode: ::prost::alloc::vec::Vec<u8>,
    /// as one instance of the service could be used for multiple chains
    #[prost(uint64, tag = "4")]
    pub chain_id: u64,
    /// should be valid input standard JSON
    #[prost(string, tag = "5")]
    pub input: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyStandardJsonResponse {
    #[prost(uint32, tag = "1")]
    pub status: u32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub result: ::core::option::Option<verify_standard_json_response::Result>,
}
/// Nested message and enum types in `VerifyStandardJsonResponse`.
pub mod verify_standard_json_response {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Result {
        #[prost(string, tag = "1")]
        pub file_path: ::prost::alloc::string::String,
        #[prost(string, tag = "2")]
        pub name: ::prost::alloc::string::String,
        #[prost(string, tag = "3")]
        pub compiler_version: ::prost::alloc::string::String,
        #[prost(string, tag = "4")]
        pub evm_version: ::prost::alloc::string::String,
        #[prost(bool, optional, tag = "5")]
        pub optimization: ::core::option::Option<bool>,
        #[prost(uint32, optional, tag = "6")]
        pub optimization_runs: ::core::option::Option<u32>,
        #[prost(map = "string, string", tag = "7")]
        pub libraries: ::std::collections::HashMap<
            ::prost::alloc::string::String,
            ::prost::alloc::string::String,
        >,
        #[prost(map = "string, string", tag = "8")]
        pub sources: ::std::collections::HashMap<
            ::prost::alloc::string::String,
            ::prost::alloc::string::String,
        >,
        #[prost(string, tag = "9")]
        pub abi: ::prost::alloc::string::String,
        #[prost(bytes = "vec", tag = "10")]
        pub constructor_arguments: ::prost::alloc::vec::Vec<u8>,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyViaSourcifyRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag = "2")]
    pub chain_id: u64,
    #[prost(map = "string, string", tag = "3")]
    pub files:
        ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    /// should be `uint32`?
    #[prost(uint64, optional, tag = "4")]
    pub chosen_contract: ::core::option::Option<u64>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyViaSourcifyResponse {
    #[prost(uint32, tag = "1")]
    pub status: u32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub result: ::core::option::Option<verify_via_sourcify_response::Result>,
}
/// Nested message and enum types in `VerifyViaSourcifyResponse`.
pub mod verify_via_sourcify_response {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Result {
        #[prost(string, tag = "1")]
        pub file_path: ::prost::alloc::string::String,
        #[prost(string, tag = "2")]
        pub name: ::prost::alloc::string::String,
        #[prost(string, tag = "3")]
        pub compiler_version: ::prost::alloc::string::String,
        #[prost(string, tag = "4")]
        pub evm_version: ::prost::alloc::string::String,
        #[prost(bool, optional, tag = "5")]
        pub optimization: ::core::option::Option<bool>,
        #[prost(uint32, optional, tag = "6")]
        pub optimization_runs: ::core::option::Option<u32>,
        #[prost(map = "string, string", tag = "7")]
        pub libraries: ::std::collections::HashMap<
            ::prost::alloc::string::String,
            ::prost::alloc::string::String,
        >,
        #[prost(map = "string, string", tag = "8")]
        pub sources: ::std::collections::HashMap<
            ::prost::alloc::string::String,
            ::prost::alloc::string::String,
        >,
        #[prost(string, tag = "9")]
        pub abi: ::prost::alloc::string::String,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyVyperRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub creation_tx_input: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub deployed_bytecode: ::prost::alloc::vec::Vec<u8>,
    /// as one instance of the service could be used for multiple chains
    #[prost(uint64, tag = "4")]
    pub chain_id: u64,
    #[prost(string, tag = "5")]
    pub compiler_version: ::prost::alloc::string::String,
    #[prost(string, tag = "6")]
    pub source_code: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyVyperResponse {
    #[prost(uint32, tag = "1")]
    pub status: u32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub result: ::core::option::Option<verify_vyper_response::Result>,
}
/// Nested message and enum types in `VerifyVyperResponse`.
pub mod verify_vyper_response {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Result {
        #[prost(string, tag = "1")]
        pub name: ::prost::alloc::string::String,
        #[prost(string, tag = "2")]
        pub compiler_version: ::prost::alloc::string::String,
        #[prost(string, tag = "3")]
        pub evm_version: ::prost::alloc::string::String,
        /// there are no optimization runs concept for Vyper
        #[prost(bool, tag = "4")]
        pub optimization: bool,
        #[prost(string, tag = "5")]
        pub source_code: ::prost::alloc::string::String,
        #[prost(string, tag = "6")]
        pub abi: ::prost::alloc::string::String,
    }
}
/// Returned as a response for all types of async verification
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyAsyncResponse {
    /// uuid
    #[prost(bytes = "vec", tag = "1")]
    pub verification_id: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetVerificationStatusRequest {
    /// uuid
    #[prost(bytes = "vec", tag = "1")]
    pub verification_id: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetVerificationStatusResponse {
    #[prost(enumeration = "get_verification_status_response::Status", tag = "1")]
    pub status: i32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
    #[prost(
        oneof = "get_verification_status_response::Result",
        tags = "3, 4, 5, 6, 7"
    )]
    pub result: ::core::option::Option<get_verification_status_response::Result>,
}
/// Nested message and enum types in `GetVerificationStatusResponse`.
pub mod get_verification_status_response {
    /// Backward compatible with current elixir impl
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
    #[repr(i32)]
    pub enum Status {
        Pending = 0,
        Pass = 1,
        Fail = 2,
        UnknownId = 3,
    }
    impl Status {
        /// String value of the enum field names used in the ProtoBuf definition.
        ///
        /// The values are not transformed in any way and thus are considered stable
        /// (if the ProtoBuf definition does not change) and safe for programmatic use.
        pub fn as_str_name(&self) -> &'static str {
            match self {
                Status::Pending => "PENDING",
                Status::Pass => "PASS",
                Status::Fail => "FAIL",
                Status::UnknownId => "UNKNOWN_ID",
            }
        }
    }
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "3")]
        SingleResult(super::verify_single_response::Result),
        #[prost(message, tag = "4")]
        MultiResult(super::verify_multi_response::Result),
        #[prost(message, tag = "5")]
        StandardJsonResult(super::verify_standard_json_response::Result),
        #[prost(message, tag = "6")]
        ViaSourcifyResult(super::verify_via_sourcify_response::Result),
        #[prost(message, tag = "7")]
        VyperResult(super::verify_vyper_response::Result),
    }
}
/// Generated client implementations.
pub mod smart_contract_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::{http::Uri, *};
    #[derive(Debug, Clone)]
    pub struct SmartContractServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl SmartContractServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> SmartContractServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> SmartContractServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<http::Request<tonic::body::BoxBody>>>::Error:
                Into<StdError> + Send + Sync,
        {
            SmartContractServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Retrieves abi from SCDB and sends it back to the caller
        pub async fn get_abi(
            &mut self,
            request: impl tonic::IntoRequest<super::GetAbiRequest>,
        ) -> Result<tonic::Response<super::GetAbiResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractService/GetAbi",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Retrieves all info from SCDB and sends it back to the caller
        pub async fn get_sources(
            &mut self,
            request: impl tonic::IntoRequest<super::GetSourcesRequest>,
        ) -> Result<tonic::Response<super::GetSourcesResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractService/GetSources",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Verifies a single-file contract
        pub async fn verify_single(
            &mut self,
            request: impl tonic::IntoRequest<super::VerifySingleRequest>,
        ) -> Result<tonic::Response<super::VerifySingleResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractService/VerifySingle",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Verifies a single-file contract
        pub async fn verify_multi(
            &mut self,
            request: impl tonic::IntoRequest<super::VerifyMultiRequest>,
        ) -> Result<tonic::Response<super::VerifyMultiResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractService/VerifyMulti",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Returns verification id to get the status later in case of async, or the
        /// verification result if sync
        pub async fn verify_standard_json(
            &mut self,
            request: impl tonic::IntoRequest<super::VerifyStandardJsonRequest>,
        ) -> Result<tonic::Response<super::VerifyStandardJsonResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractService/VerifyStandardJson",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Verifies a contract via sourcify metadata
        pub async fn verify_via_sourcify(
            &mut self,
            request: impl tonic::IntoRequest<super::VerifyViaSourcifyRequest>,
        ) -> Result<tonic::Response<super::VerifyViaSourcifyResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractService/VerifyViaSourcify",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Verifies vyper contracts
        pub async fn verify_vyper(
            &mut self,
            request: impl tonic::IntoRequest<super::VerifyVyperRequest>,
        ) -> Result<tonic::Response<super::VerifyVyperResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractService/VerifyVyper",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
/// Generated client implementations.
pub mod smart_contract_async_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::{http::Uri, *};
    /// Async verification related endpoints
    #[derive(Debug, Clone)]
    pub struct SmartContractAsyncServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl SmartContractAsyncServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: std::convert::TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> SmartContractAsyncServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> SmartContractAsyncServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<http::Request<tonic::body::BoxBody>>>::Error:
                Into<StdError> + Send + Sync,
        {
            SmartContractAsyncServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Async version of `VerifySingle`
        pub async fn verify_single_async(
            &mut self,
            request: impl tonic::IntoRequest<super::VerifySingleRequest>,
        ) -> Result<tonic::Response<super::VerifyAsyncResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractAsyncService/VerifySingleAsync",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Async version of `VerifySingle`
        pub async fn verify_multi_async(
            &mut self,
            request: impl tonic::IntoRequest<super::VerifyMultiRequest>,
        ) -> Result<tonic::Response<super::VerifyAsyncResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractAsyncService/VerifyMultiAsync",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Async version of `VerifyStandardJson`
        pub async fn verify_standard_json_async(
            &mut self,
            request: impl tonic::IntoRequest<super::VerifyStandardJsonRequest>,
        ) -> Result<tonic::Response<super::VerifyAsyncResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractAsyncService/VerifyStandardJsonAsync",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Async version of `VerifyViaSourcify`
        pub async fn verify_via_sourcify_async(
            &mut self,
            request: impl tonic::IntoRequest<super::VerifyViaSourcifyRequest>,
        ) -> Result<tonic::Response<super::VerifyAsyncResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractAsyncService/VerifyViaSourcifyAsync",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Async version of `VerifyStandardJson`
        pub async fn verify_vyper_async(
            &mut self,
            request: impl tonic::IntoRequest<super::VerifyVyperRequest>,
        ) -> Result<tonic::Response<super::VerifyAsyncResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractAsyncService/VerifyVyperAsync",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
        /// Accepts verification id and returns status with info
        pub async fn get_verification_status(
            &mut self,
            request: impl tonic::IntoRequest<super::GetVerificationStatusRequest>,
        ) -> Result<tonic::Response<super::GetVerificationStatusResponse>, tonic::Status> {
            self.inner.ready().await.map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unknown,
                    format!("Service was not ready: {}", e.into()),
                )
            })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/smart_contract_service.SmartContractAsyncService/GetVerificationStatus",
            );
            self.inner.unary(request.into_request(), path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod smart_contract_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    ///Generated trait containing gRPC methods that should be implemented for use with SmartContractServiceServer.
    #[async_trait]
    pub trait SmartContractService: Send + Sync + 'static {
        /// Retrieves abi from SCDB and sends it back to the caller
        async fn get_abi(
            &self,
            request: tonic::Request<super::GetAbiRequest>,
        ) -> Result<tonic::Response<super::GetAbiResponse>, tonic::Status>;
        /// Retrieves all info from SCDB and sends it back to the caller
        async fn get_sources(
            &self,
            request: tonic::Request<super::GetSourcesRequest>,
        ) -> Result<tonic::Response<super::GetSourcesResponse>, tonic::Status>;
        /// Verifies a single-file contract
        async fn verify_single(
            &self,
            request: tonic::Request<super::VerifySingleRequest>,
        ) -> Result<tonic::Response<super::VerifySingleResponse>, tonic::Status>;
        /// Verifies a single-file contract
        async fn verify_multi(
            &self,
            request: tonic::Request<super::VerifyMultiRequest>,
        ) -> Result<tonic::Response<super::VerifyMultiResponse>, tonic::Status>;
        /// Returns verification id to get the status later in case of async, or the
        /// verification result if sync
        async fn verify_standard_json(
            &self,
            request: tonic::Request<super::VerifyStandardJsonRequest>,
        ) -> Result<tonic::Response<super::VerifyStandardJsonResponse>, tonic::Status>;
        /// Verifies a contract via sourcify metadata
        async fn verify_via_sourcify(
            &self,
            request: tonic::Request<super::VerifyViaSourcifyRequest>,
        ) -> Result<tonic::Response<super::VerifyViaSourcifyResponse>, tonic::Status>;
        /// Verifies vyper contracts
        async fn verify_vyper(
            &self,
            request: tonic::Request<super::VerifyVyperRequest>,
        ) -> Result<tonic::Response<super::VerifyVyperResponse>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct SmartContractServiceServer<T: SmartContractService> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: SmartContractService> SmartContractServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
            }
        }
        pub fn with_interceptor<F>(inner: T, interceptor: F) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for SmartContractServiceServer<T>
    where
        T: SmartContractService,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/smart_contract_service.SmartContractService/GetAbi" => {
                    #[allow(non_camel_case_types)]
                    struct GetAbiSvc<T: SmartContractService>(pub Arc<T>);
                    impl<T: SmartContractService> tonic::server::UnaryService<super::GetAbiRequest> for GetAbiSvc<T> {
                        type Response = super::GetAbiResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetAbiRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).get_abi(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetAbiSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/smart_contract_service.SmartContractService/GetSources" => {
                    #[allow(non_camel_case_types)]
                    struct GetSourcesSvc<T: SmartContractService>(pub Arc<T>);
                    impl<T: SmartContractService>
                        tonic::server::UnaryService<super::GetSourcesRequest> for GetSourcesSvc<T>
                    {
                        type Response = super::GetSourcesResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetSourcesRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).get_sources(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetSourcesSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/smart_contract_service.SmartContractService/VerifySingle" => {
                    #[allow(non_camel_case_types)]
                    struct VerifySingleSvc<T: SmartContractService>(pub Arc<T>);
                    impl<T: SmartContractService>
                        tonic::server::UnaryService<super::VerifySingleRequest>
                        for VerifySingleSvc<T>
                    {
                        type Response = super::VerifySingleResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::VerifySingleRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).verify_single(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = VerifySingleSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/smart_contract_service.SmartContractService/VerifyMulti" => {
                    #[allow(non_camel_case_types)]
                    struct VerifyMultiSvc<T: SmartContractService>(pub Arc<T>);
                    impl<T: SmartContractService>
                        tonic::server::UnaryService<super::VerifyMultiRequest>
                        for VerifyMultiSvc<T>
                    {
                        type Response = super::VerifyMultiResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::VerifyMultiRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).verify_multi(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = VerifyMultiSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/smart_contract_service.SmartContractService/VerifyStandardJson" => {
                    #[allow(non_camel_case_types)]
                    struct VerifyStandardJsonSvc<T: SmartContractService>(pub Arc<T>);
                    impl<T: SmartContractService>
                        tonic::server::UnaryService<super::VerifyStandardJsonRequest>
                        for VerifyStandardJsonSvc<T>
                    {
                        type Response = super::VerifyStandardJsonResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::VerifyStandardJsonRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).verify_standard_json(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = VerifyStandardJsonSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/smart_contract_service.SmartContractService/VerifyViaSourcify" => {
                    #[allow(non_camel_case_types)]
                    struct VerifyViaSourcifySvc<T: SmartContractService>(pub Arc<T>);
                    impl<T: SmartContractService>
                        tonic::server::UnaryService<super::VerifyViaSourcifyRequest>
                        for VerifyViaSourcifySvc<T>
                    {
                        type Response = super::VerifyViaSourcifyResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::VerifyViaSourcifyRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).verify_via_sourcify(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = VerifyViaSourcifySvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/smart_contract_service.SmartContractService/VerifyVyper" => {
                    #[allow(non_camel_case_types)]
                    struct VerifyVyperSvc<T: SmartContractService>(pub Arc<T>);
                    impl<T: SmartContractService>
                        tonic::server::UnaryService<super::VerifyVyperRequest>
                        for VerifyVyperSvc<T>
                    {
                        type Response = super::VerifyVyperResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::VerifyVyperRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).verify_vyper(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = VerifyVyperSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .header("content-type", "application/grpc")
                        .body(empty_body())
                        .unwrap())
                }),
            }
        }
    }
    impl<T: SmartContractService> Clone for SmartContractServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
            }
        }
    }
    impl<T: SmartContractService> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: SmartContractService> tonic::server::NamedService for SmartContractServiceServer<T> {
        const NAME: &'static str = "smart_contract_service.SmartContractService";
    }
}
/// Generated server implementations.
pub mod smart_contract_async_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    ///Generated trait containing gRPC methods that should be implemented for use with SmartContractAsyncServiceServer.
    #[async_trait]
    pub trait SmartContractAsyncService: Send + Sync + 'static {
        /// Async version of `VerifySingle`
        async fn verify_single_async(
            &self,
            request: tonic::Request<super::VerifySingleRequest>,
        ) -> Result<tonic::Response<super::VerifyAsyncResponse>, tonic::Status>;
        /// Async version of `VerifySingle`
        async fn verify_multi_async(
            &self,
            request: tonic::Request<super::VerifyMultiRequest>,
        ) -> Result<tonic::Response<super::VerifyAsyncResponse>, tonic::Status>;
        /// Async version of `VerifyStandardJson`
        async fn verify_standard_json_async(
            &self,
            request: tonic::Request<super::VerifyStandardJsonRequest>,
        ) -> Result<tonic::Response<super::VerifyAsyncResponse>, tonic::Status>;
        /// Async version of `VerifyViaSourcify`
        async fn verify_via_sourcify_async(
            &self,
            request: tonic::Request<super::VerifyViaSourcifyRequest>,
        ) -> Result<tonic::Response<super::VerifyAsyncResponse>, tonic::Status>;
        /// Async version of `VerifyStandardJson`
        async fn verify_vyper_async(
            &self,
            request: tonic::Request<super::VerifyVyperRequest>,
        ) -> Result<tonic::Response<super::VerifyAsyncResponse>, tonic::Status>;
        /// Accepts verification id and returns status with info
        async fn get_verification_status(
            &self,
            request: tonic::Request<super::GetVerificationStatusRequest>,
        ) -> Result<tonic::Response<super::GetVerificationStatusResponse>, tonic::Status>;
    }
    /// Async verification related endpoints
    #[derive(Debug)]
    pub struct SmartContractAsyncServiceServer<T: SmartContractAsyncService> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: SmartContractAsyncService> SmartContractAsyncServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
            }
        }
        pub fn with_interceptor<F>(inner: T, interceptor: F) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for SmartContractAsyncServiceServer<T>
    where
        T: SmartContractAsyncService,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/smart_contract_service.SmartContractAsyncService/VerifySingleAsync" => {
                    #[allow(non_camel_case_types)]
                    struct VerifySingleAsyncSvc<T: SmartContractAsyncService>(pub Arc<T>);
                    impl<T: SmartContractAsyncService>
                        tonic::server::UnaryService<super::VerifySingleRequest>
                        for VerifySingleAsyncSvc<T>
                    {
                        type Response = super::VerifyAsyncResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::VerifySingleRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).verify_single_async(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = VerifySingleAsyncSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/smart_contract_service.SmartContractAsyncService/VerifyMultiAsync" => {
                    #[allow(non_camel_case_types)]
                    struct VerifyMultiAsyncSvc<T: SmartContractAsyncService>(pub Arc<T>);
                    impl<T: SmartContractAsyncService>
                        tonic::server::UnaryService<super::VerifyMultiRequest>
                        for VerifyMultiAsyncSvc<T>
                    {
                        type Response = super::VerifyAsyncResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::VerifyMultiRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).verify_multi_async(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = VerifyMultiAsyncSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/smart_contract_service.SmartContractAsyncService/VerifyStandardJsonAsync" => {
                    #[allow(non_camel_case_types)]
                    struct VerifyStandardJsonAsyncSvc<T: SmartContractAsyncService>(pub Arc<T>);
                    impl<T: SmartContractAsyncService>
                        tonic::server::UnaryService<super::VerifyStandardJsonRequest>
                        for VerifyStandardJsonAsyncSvc<T>
                    {
                        type Response = super::VerifyAsyncResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::VerifyStandardJsonRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut =
                                async move { (*inner).verify_standard_json_async(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = VerifyStandardJsonAsyncSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/smart_contract_service.SmartContractAsyncService/VerifyViaSourcifyAsync" => {
                    #[allow(non_camel_case_types)]
                    struct VerifyViaSourcifyAsyncSvc<T: SmartContractAsyncService>(pub Arc<T>);
                    impl<T: SmartContractAsyncService>
                        tonic::server::UnaryService<super::VerifyViaSourcifyRequest>
                        for VerifyViaSourcifyAsyncSvc<T>
                    {
                        type Response = super::VerifyAsyncResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::VerifyViaSourcifyRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut =
                                async move { (*inner).verify_via_sourcify_async(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = VerifyViaSourcifyAsyncSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/smart_contract_service.SmartContractAsyncService/VerifyVyperAsync" => {
                    #[allow(non_camel_case_types)]
                    struct VerifyVyperAsyncSvc<T: SmartContractAsyncService>(pub Arc<T>);
                    impl<T: SmartContractAsyncService>
                        tonic::server::UnaryService<super::VerifyVyperRequest>
                        for VerifyVyperAsyncSvc<T>
                    {
                        type Response = super::VerifyAsyncResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::VerifyVyperRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut = async move { (*inner).verify_vyper_async(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = VerifyVyperAsyncSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/smart_contract_service.SmartContractAsyncService/GetVerificationStatus" => {
                    #[allow(non_camel_case_types)]
                    struct GetVerificationStatusSvc<T: SmartContractAsyncService>(pub Arc<T>);
                    impl<T: SmartContractAsyncService>
                        tonic::server::UnaryService<super::GetVerificationStatusRequest>
                        for GetVerificationStatusSvc<T>
                    {
                        type Response = super::GetVerificationStatusResponse;
                        type Future = BoxFuture<tonic::Response<Self::Response>, tonic::Status>;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetVerificationStatusRequest>,
                        ) -> Self::Future {
                            let inner = self.0.clone();
                            let fut =
                                async move { (*inner).get_verification_status(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetVerificationStatusSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec).apply_compression_config(
                            accept_compression_encodings,
                            send_compression_encodings,
                        );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => Box::pin(async move {
                    Ok(http::Response::builder()
                        .status(200)
                        .header("grpc-status", "12")
                        .header("content-type", "application/grpc")
                        .body(empty_body())
                        .unwrap())
                }),
            }
        }
    }
    impl<T: SmartContractAsyncService> Clone for SmartContractAsyncServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
            }
        }
    }
    impl<T: SmartContractAsyncService> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: SmartContractAsyncService> tonic::server::NamedService
        for SmartContractAsyncServiceServer<T>
    {
        const NAME: &'static str = "smart_contract_service.SmartContractAsyncService";
    }
}
