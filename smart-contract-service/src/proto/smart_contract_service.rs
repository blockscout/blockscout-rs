#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAbiRequest {
    #[prost(bytes="vec", tag="1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag="2")]
    pub chain_id: u64,
}
/// todo!() 
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetAbiResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSourcesRequest {
    #[prost(bytes="vec", tag="1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag="2")]
    pub chain_id: u64,
}
/// todo!() 
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetSourcesResponse {
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifySingleRequest {
    #[prost(bytes="vec", tag="1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes="vec", tag="2")]
    pub creation_tx_input: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes="vec", tag="3")]
    pub deployed_bytecode: ::prost::alloc::vec::Vec<u8>,
    /// as one instance of the service could be used for multiple chains
    #[prost(uint64, tag="4")]
    pub chain_id: u64,
    #[prost(string, tag="5")]
    pub compiler_version: ::prost::alloc::string::String,
    /// if None, `evm_version` is set to "default"
    #[prost(string, optional, tag="6")]
    pub evm_version: ::core::option::Option<::prost::alloc::string::String>,
    /// if None, `optimizer.enabled` is set to be false
    #[prost(uint32, optional, tag="7")]
    pub optimization_runs: ::core::option::Option<u32>,
    /// library_name -> library_address;
    #[prost(map="string, string", tag="8")]
    pub libraries: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    #[prost(string, tag="9")]
    pub source_code: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifySingleResponse {
    #[prost(uint32, tag="1")]
    pub status: u32,
    #[prost(string, tag="2")]
    pub message: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub result: ::core::option::Option<verify_single_response::Result>,
}
/// Nested message and enum types in `VerifySingleResponse`.
pub mod verify_single_response {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Result {
        #[prost(string, tag="1")]
        pub name: ::prost::alloc::string::String,
        #[prost(string, tag="2")]
        pub compiler_version: ::prost::alloc::string::String,
        #[prost(string, tag="3")]
        pub evm_version: ::prost::alloc::string::String,
        #[prost(bool, optional, tag="4")]
        pub optimization: ::core::option::Option<bool>,
        #[prost(uint32, optional, tag="5")]
        pub optimization_runs: ::core::option::Option<u32>,
        #[prost(map="string, string", tag="6")]
        pub libraries: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
        #[prost(string, tag="7")]
        pub source_code: ::prost::alloc::string::String,
        #[prost(string, tag="9")]
        pub abi: ::prost::alloc::string::String,
        #[prost(bytes="vec", tag="10")]
        pub constructor_arguments: ::prost::alloc::vec::Vec<u8>,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyMultiRequest {
    #[prost(bytes="vec", tag="1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes="vec", tag="2")]
    pub creation_tx_input: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes="vec", tag="3")]
    pub deployed_bytecode: ::prost::alloc::vec::Vec<u8>,
    /// as one instance of the service could be used for multiple chains
    #[prost(uint64, tag="4")]
    pub chain_id: u64,
    #[prost(string, tag="5")]
    pub compiler_version: ::prost::alloc::string::String,
    /// if None, `evm_version` is set to "default"
    #[prost(string, optional, tag="6")]
    pub evm_version: ::core::option::Option<::prost::alloc::string::String>,
    /// if None, `optimizer.enabled` is set to be false
    #[prost(uint32, optional, tag="7")]
    pub optimization_runs: ::core::option::Option<u32>,
    /// library_name -> library_address;
    #[prost(map="string, string", tag="8")]
    pub libraries: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    #[prost(map="string, string", tag="9")]
    pub sources: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyMultiResponse {
    #[prost(uint32, tag="1")]
    pub status: u32,
    #[prost(string, tag="2")]
    pub message: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub result: ::core::option::Option<verify_multi_response::Result>,
}
/// Nested message and enum types in `VerifyMultiResponse`.
pub mod verify_multi_response {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Result {
        #[prost(string, tag="1")]
        pub file_path: ::prost::alloc::string::String,
        #[prost(string, tag="2")]
        pub name: ::prost::alloc::string::String,
        #[prost(string, tag="3")]
        pub compiler_version: ::prost::alloc::string::String,
        #[prost(string, tag="4")]
        pub evm_version: ::prost::alloc::string::String,
        #[prost(bool, optional, tag="5")]
        pub optimization: ::core::option::Option<bool>,
        #[prost(uint32, optional, tag="6")]
        pub optimization_runs: ::core::option::Option<u32>,
        #[prost(map="string, string", tag="7")]
        pub libraries: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
        #[prost(map="string, string", tag="8")]
        pub sources: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
        #[prost(string, tag="9")]
        pub abi: ::prost::alloc::string::String,
        #[prost(bytes="vec", tag="10")]
        pub constructor_arguments: ::prost::alloc::vec::Vec<u8>,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyStandardJsonRequest {
    #[prost(bytes="vec", tag="1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes="vec", tag="2")]
    pub creation_tx_input: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes="vec", tag="3")]
    pub deployed_bytecode: ::prost::alloc::vec::Vec<u8>,
    /// as one instance of the service could be used for multiple chains
    #[prost(uint64, tag="4")]
    pub chain_id: u64,
    /// should be valid input standard JSON
    #[prost(string, tag="5")]
    pub input: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyStandardJsonResponse {
    #[prost(uint32, tag="1")]
    pub status: u32,
    #[prost(string, tag="2")]
    pub message: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub result: ::core::option::Option<verify_standard_json_response::Result>,
}
/// Nested message and enum types in `VerifyStandardJsonResponse`.
pub mod verify_standard_json_response {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Result {
        #[prost(string, tag="1")]
        pub file_path: ::prost::alloc::string::String,
        #[prost(string, tag="2")]
        pub name: ::prost::alloc::string::String,
        #[prost(string, tag="3")]
        pub compiler_version: ::prost::alloc::string::String,
        #[prost(string, tag="4")]
        pub evm_version: ::prost::alloc::string::String,
        #[prost(bool, optional, tag="5")]
        pub optimization: ::core::option::Option<bool>,
        #[prost(uint32, optional, tag="6")]
        pub optimization_runs: ::core::option::Option<u32>,
        #[prost(map="string, string", tag="7")]
        pub libraries: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
        #[prost(map="string, string", tag="8")]
        pub sources: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
        #[prost(string, tag="9")]
        pub abi: ::prost::alloc::string::String,
        #[prost(bytes="vec", tag="10")]
        pub constructor_arguments: ::prost::alloc::vec::Vec<u8>,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyViaSourcifyRequest {
    #[prost(bytes="vec", tag="1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(uint64, tag="2")]
    pub chain_id: u64,
    #[prost(map="string, string", tag="3")]
    pub files: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
    /// should be `uint32`?
    #[prost(uint64, optional, tag="4")]
    pub chosen_contract: ::core::option::Option<u64>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyViaSourcifyResponse {
    #[prost(uint32, tag="1")]
    pub status: u32,
    #[prost(string, tag="2")]
    pub message: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub result: ::core::option::Option<verify_via_sourcify_response::Result>,
}
/// Nested message and enum types in `VerifyViaSourcifyResponse`.
pub mod verify_via_sourcify_response {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Result {
        #[prost(string, tag="1")]
        pub file_path: ::prost::alloc::string::String,
        #[prost(string, tag="2")]
        pub name: ::prost::alloc::string::String,
        #[prost(string, tag="3")]
        pub compiler_version: ::prost::alloc::string::String,
        #[prost(string, tag="4")]
        pub evm_version: ::prost::alloc::string::String,
        #[prost(bool, optional, tag="5")]
        pub optimization: ::core::option::Option<bool>,
        #[prost(uint32, optional, tag="6")]
        pub optimization_runs: ::core::option::Option<u32>,
        #[prost(map="string, string", tag="7")]
        pub libraries: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
        #[prost(map="string, string", tag="8")]
        pub sources: ::std::collections::HashMap<::prost::alloc::string::String, ::prost::alloc::string::String>,
        #[prost(string, tag="9")]
        pub abi: ::prost::alloc::string::String,
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyVyperRequest {
    #[prost(bytes="vec", tag="1")]
    pub contract_address: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes="vec", tag="2")]
    pub creation_tx_input: ::prost::alloc::vec::Vec<u8>,
    #[prost(bytes="vec", tag="3")]
    pub deployed_bytecode: ::prost::alloc::vec::Vec<u8>,
    /// as one instance of the service could be used for multiple chains
    #[prost(uint64, tag="4")]
    pub chain_id: u64,
    #[prost(string, tag="5")]
    pub compiler_version: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub source_code: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyVyperResponse {
    #[prost(uint32, tag="1")]
    pub status: u32,
    #[prost(string, tag="2")]
    pub message: ::prost::alloc::string::String,
    #[prost(message, optional, tag="3")]
    pub result: ::core::option::Option<verify_vyper_response::Result>,
}
/// Nested message and enum types in `VerifyVyperResponse`.
pub mod verify_vyper_response {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Result {
        #[prost(string, tag="1")]
        pub name: ::prost::alloc::string::String,
        #[prost(string, tag="2")]
        pub compiler_version: ::prost::alloc::string::String,
        #[prost(string, tag="3")]
        pub evm_version: ::prost::alloc::string::String,
        /// there are no optimization runs concept for Vyper
        #[prost(bool, tag="4")]
        pub optimization: bool,
        #[prost(string, tag="5")]
        pub source_code: ::prost::alloc::string::String,
        #[prost(string, tag="6")]
        pub abi: ::prost::alloc::string::String,
    }
}
/// Returned as a response for all types of async verification
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct VerifyAsyncResponse {
    /// uuid
    #[prost(bytes="vec", tag="1")]
    pub verification_id: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetVerificationStatusRequest {
    /// uuid
    #[prost(bytes="vec", tag="1")]
    pub verification_id: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetVerificationStatusResponse {
    #[prost(enumeration="get_verification_status_response::Status", tag="1")]
    pub status: i32,
    #[prost(string, tag="2")]
    pub message: ::prost::alloc::string::String,
    #[prost(oneof="get_verification_status_response::Result", tags="3, 4, 5, 6, 7")]
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
        #[prost(message, tag="3")]
        SingleResult(super::verify_single_response::Result),
        #[prost(message, tag="4")]
        MultiResult(super::verify_multi_response::Result),
        #[prost(message, tag="5")]
        StandardJsonResult(super::verify_standard_json_response::Result),
        #[prost(message, tag="6")]
        ViaSourcifyResult(super::verify_via_sourcify_response::Result),
        #[prost(message, tag="7")]
        VyperResult(super::verify_vyper_response::Result),
    }
}
/// Generated client implementations.
pub mod smart_contract_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
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
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
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
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
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
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            SmartContractAsyncServiceClient::new(
                InterceptedService::new(inner, interceptor),
            )
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
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
        ) -> Result<
            tonic::Response<super::GetVerificationStatusResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
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
