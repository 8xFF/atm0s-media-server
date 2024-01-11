use std::fmt::Debug;

pub mod connector;
pub mod gateway;
pub mod general;
pub mod sip;
pub mod webrtc;
pub mod whep;
pub mod whip;

pub trait RpcRequest: Send {
    fn cmd(&self) -> &str;
    /// Parse into param, if cannot it auto reply with DESERIALIZE_ERROR
    fn parse<P: for<'a> TryFrom<&'a [u8]> + Send + 'static, R: Into<Vec<u8>> + Send + 'static>(self) -> Option<Box<dyn RpcReqRes<P, R>>>;
    /// Answer error
    fn error(self, err: &str);
}

pub trait RpcReqRes<Param, Res>: Debug + Send {
    fn param(&self) -> &Param;
    fn answer(&self, res: Result<Res, &str>);
}

impl<P: PartialEq, R> Eq for Box<dyn RpcReqRes<P, R>> {}

impl<P: PartialEq, R> PartialEq for Box<dyn RpcReqRes<P, R>> {
    fn eq(&self, other: &Self) -> bool {
        self.param().eq(other.param())
    }
}

#[async_trait::async_trait]
pub trait RpcEndpoint<Req: RpcRequest, Emitter: RpcEmitter> {
    fn emitter(&mut self) -> Emitter;
    async fn recv(&mut self) -> Option<Req>;
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum RpcError {
    Timeout,
    LocalQueueError,
    RemoteQueueError,
    DeserializeError,
    RuntimeError(String),
}

#[async_trait::async_trait]
pub trait RpcEmitter: Clone {
    fn emit<E: Into<Vec<u8>>>(&self, service: u8, node: Option<u32>, cmd: &str, event: E);
    async fn request<Req: Into<Vec<u8>> + Send, Res: for<'a> TryFrom<&'a [u8]> + Send>(&self, service: u8, node: Option<u32>, cmd: &str, req: Req, timeout_ms: u64) -> Result<Res, RpcError>;
}

pub const RPC_WEBRTC_CONNECT: &str = "RPC_WEBRTC_CONNECT";
pub const RPC_WEBRTC_ICE: &str = "RPC_WEBRTC_ICE";
pub const RPC_WEBRTC_PATCH: &str = "RPC_WEBRTC_PATCH";
pub const RPC_MEDIA_ENDPOINT_CLOSE: &str = "RPC_MEDIA_ENDPOINT_CLOSE";
pub const RPC_WHIP_CONNECT: &str = "RPC_WHIP_CONNECT";
pub const RPC_WHEP_CONNECT: &str = "RPC_WHEP_CONNECT";

pub const RPC_NODE_PING: &str = "RPC_NODE_PING";
pub const RPC_NODE_HEALTHCHECK: &str = "RPC_NODE_HEALTHCHECK";

pub const RPC_MEDIA_ENDPOINT_LOG: &str = "RPC_MEDIA_ENDPOINT_LOG";
