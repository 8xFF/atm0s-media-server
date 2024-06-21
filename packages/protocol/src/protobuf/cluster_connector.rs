// This file is @generated by prost-build.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConnectorRequest {
    #[prost(uint64, tag = "1")]
    pub req_id: u64,
    #[prost(uint64, tag = "2")]
    pub ts: u64,
    #[prost(oneof = "connector_request::Event", tags = "3")]
    pub event: ::core::option::Option<connector_request::Event>,
}
/// Nested message and enum types in `ConnectorRequest`.
pub mod connector_request {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Event {
        #[prost(message, tag = "3")]
        Peer(super::PeerEvent),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ConnectorResponse {
    #[prost(uint64, tag = "1")]
    pub req_id: u64,
    #[prost(oneof = "connector_response::Response", tags = "2, 3")]
    pub response: ::core::option::Option<connector_response::Response>,
}
/// Nested message and enum types in `ConnectorResponse`.
pub mod connector_response {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Success {}
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Error {
        #[prost(uint32, tag = "1")]
        pub code: u32,
        #[prost(string, tag = "2")]
        pub message: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Response {
        #[prost(message, tag = "2")]
        Success(Success),
        #[prost(message, tag = "3")]
        Error(Error),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PeerEvent {
    #[prost(uint64, tag = "1")]
    pub session_id: u64,
    #[prost(oneof = "peer_event::Event", tags = "2, 3, 4, 5, 6, 7, 8, 9, 10, 11")]
    pub event: ::core::option::Option<peer_event::Event>,
}
/// Nested message and enum types in `PeerEvent`.
pub mod peer_event {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct RouteBegin {
        #[prost(uint32, tag = "1")]
        pub dest_node: u32,
        #[prost(string, tag = "2")]
        pub ip_addr: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct RouteSuccess {
        #[prost(uint32, tag = "1")]
        pub after_ms: u32,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct RouteError {
        #[prost(uint32, tag = "1")]
        pub after_ms: u32,
        #[prost(enumeration = "route_error::ErrorType", tag = "2")]
        pub error: i32,
    }
    /// Nested message and enum types in `RouteError`.
    pub mod route_error {
        #[derive(
            Clone,
            Copy,
            Debug,
            PartialEq,
            Eq,
            Hash,
            PartialOrd,
            Ord,
            ::prost::Enumeration
        )]
        #[repr(i32)]
        pub enum ErrorType {
            PoolEmpty = 0,
            Timeout = 1,
        }
        impl ErrorType {
            /// String value of the enum field names used in the ProtoBuf definition.
            ///
            /// The values are not transformed in any way and thus are considered stable
            /// (if the ProtoBuf definition does not change) and safe for programmatic use.
            pub fn as_str_name(&self) -> &'static str {
                match self {
                    ErrorType::PoolEmpty => "PoolEmpty",
                    ErrorType::Timeout => "Timeout",
                }
            }
            /// Creates an enum from field names used in the ProtoBuf definition.
            pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
                match value {
                    "PoolEmpty" => Some(Self::PoolEmpty),
                    "Timeout" => Some(Self::Timeout),
                    _ => None,
                }
            }
        }
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Connecting {
        #[prost(string, tag = "1")]
        pub user_agent: ::prost::alloc::string::String,
        #[prost(string, tag = "2")]
        pub ip_addr: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct ConnectError {}
    /// Nested message and enum types in `ConnectError`.
    pub mod connect_error {
        #[derive(
            Clone,
            Copy,
            Debug,
            PartialEq,
            Eq,
            Hash,
            PartialOrd,
            Ord,
            ::prost::Enumeration
        )]
        #[repr(i32)]
        pub enum ErrorType {
            InvalidSdp = 0,
            Timeout = 1,
        }
        impl ErrorType {
            /// String value of the enum field names used in the ProtoBuf definition.
            ///
            /// The values are not transformed in any way and thus are considered stable
            /// (if the ProtoBuf definition does not change) and safe for programmatic use.
            pub fn as_str_name(&self) -> &'static str {
                match self {
                    ErrorType::InvalidSdp => "InvalidSdp",
                    ErrorType::Timeout => "Timeout",
                }
            }
            /// Creates an enum from field names used in the ProtoBuf definition.
            pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
                match value {
                    "InvalidSdp" => Some(Self::InvalidSdp),
                    "Timeout" => Some(Self::Timeout),
                    _ => None,
                }
            }
        }
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Connected {
        #[prost(uint32, tag = "1")]
        pub after_ms: u32,
        #[prost(string, tag = "2")]
        pub remote_ip: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Stats {
        #[prost(uint64, tag = "1")]
        pub sent_bytes: u64,
        #[prost(uint64, tag = "2")]
        pub received_bytes: u64,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Reconnecting {
        #[prost(string, tag = "1")]
        pub remote_ip: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Reconnected {
        #[prost(uint32, tag = "1")]
        pub after_ms: u32,
        #[prost(string, tag = "2")]
        pub remote_ip: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Disconnected {
        #[prost(uint32, tag = "1")]
        pub duration_ms: u32,
        #[prost(enumeration = "disconnected::Reason", tag = "2")]
        pub reason: i32,
    }
    /// Nested message and enum types in `Disconnected`.
    pub mod disconnected {
        #[derive(
            Clone,
            Copy,
            Debug,
            PartialEq,
            Eq,
            Hash,
            PartialOrd,
            Ord,
            ::prost::Enumeration
        )]
        #[repr(i32)]
        pub enum Reason {
            UserAction = 0,
            Timeout = 1,
            NodeShutdown = 2,
            KickByApi = 3,
        }
        impl Reason {
            /// String value of the enum field names used in the ProtoBuf definition.
            ///
            /// The values are not transformed in any way and thus are considered stable
            /// (if the ProtoBuf definition does not change) and safe for programmatic use.
            pub fn as_str_name(&self) -> &'static str {
                match self {
                    Reason::UserAction => "UserAction",
                    Reason::Timeout => "Timeout",
                    Reason::NodeShutdown => "NodeShutdown",
                    Reason::KickByApi => "KickByAPI",
                }
            }
            /// Creates an enum from field names used in the ProtoBuf definition.
            pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
                match value {
                    "UserAction" => Some(Self::UserAction),
                    "Timeout" => Some(Self::Timeout),
                    "NodeShutdown" => Some(Self::NodeShutdown),
                    "KickByAPI" => Some(Self::KickByApi),
                    _ => None,
                }
            }
        }
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Event {
        #[prost(message, tag = "2")]
        RouteBegin(RouteBegin),
        #[prost(message, tag = "3")]
        RouteSuccess(RouteSuccess),
        #[prost(message, tag = "4")]
        RouteError(RouteError),
        #[prost(message, tag = "5")]
        Connecting(Connecting),
        #[prost(message, tag = "6")]
        Connected(Connected),
        #[prost(message, tag = "7")]
        ConnectError(ConnectError),
        #[prost(message, tag = "8")]
        Stats(Stats),
        #[prost(message, tag = "9")]
        Reconnect(Reconnecting),
        #[prost(message, tag = "10")]
        Reconnected(Reconnected),
        #[prost(message, tag = "11")]
        Disconnected(Disconnected),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Empty {}
#[allow(async_fn_in_trait)]
pub trait MediaConnectorServiceHandler<CTX> {
    async fn hello(&self, ctx: &CTX, req: Empty) -> Option<Empty>;
}
pub struct MediaConnectorServiceClient<
    D,
    C: crate::rpc::RpcClient<D, S>,
    S: crate::rpc::RpcStream,
> {
    client: C,
    _tmp: std::marker::PhantomData<(D, S)>,
}
impl<D, C: crate::rpc::RpcClient<D, S>, S: crate::rpc::RpcStream> Clone
for MediaConnectorServiceClient<D, C, S> {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            _tmp: Default::default(),
        }
    }
}
impl<
    D,
    C: crate::rpc::RpcClient<D, S>,
    S: crate::rpc::RpcStream,
> MediaConnectorServiceClient<D, C, S> {
    pub fn new(client: C) -> Self {
        Self {
            client,
            _tmp: Default::default(),
        }
    }
    pub async fn hello(&self, dest: D, req: Empty) -> Option<Empty> {
        use prost::Message;
        let mut stream = self.client.connect(dest, "hello.service").await?;
        let out_buf = req.encode_to_vec();
        stream.write(&out_buf).await?;
        let in_buf = stream.read().await?;
        Empty::decode(in_buf.as_slice()).ok()
    }
}
pub struct MediaConnectorServiceServer<
    CTX,
    H: MediaConnectorServiceHandler<CTX>,
    Sr: crate::rpc::RpcServer<S>,
    S: crate::rpc::RpcStream,
> {
    ctx: std::sync::Arc<CTX>,
    handler: std::sync::Arc<H>,
    server: Sr,
    _tmp: std::marker::PhantomData<S>,
}
impl<
    CTX: 'static + Clone,
    H: 'static + MediaConnectorServiceHandler<CTX>,
    Sr: crate::rpc::RpcServer<S>,
    S: 'static + crate::rpc::RpcStream,
> MediaConnectorServiceServer<CTX, H, Sr, S> {
    pub fn new(server: Sr, ctx: CTX, handler: H) -> Self {
        Self {
            ctx: std::sync::Arc::new(ctx),
            handler: std::sync::Arc::new(handler),
            server,
            _tmp: Default::default(),
        }
    }
    pub async fn run(&mut self) {
        let local = tokio::task::LocalSet::new();
        local
            .run_until(async move {
                self.run_local().await;
            })
            .await;
    }
    async fn run_local(&mut self) {
        use prost::Message;
        while let Some((domain, mut stream)) = self.server.accept().await {
            let ctx = self.ctx.clone();
            let handler = self.handler.clone();
            match domain.as_str() {
                "hello.service" => {
                    tokio::task::spawn_local(async move {
                        if let Some(in_buf) = stream.read().await {
                            if let Ok(req) = Empty::decode(in_buf.as_slice()) {
                                if let Some(res) = handler.hello(&ctx, req).await {
                                    let out_buf = res.encode_to_vec();
                                    stream.write(&out_buf).await;
                                    stream.close().await;
                                }
                            }
                        }
                    });
                }
                _ => {}
            }
        }
    }
}