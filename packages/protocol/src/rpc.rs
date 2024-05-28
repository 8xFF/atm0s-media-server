use std::net::{SocketAddr, SocketAddrV4};

#[cfg(feature = "quinn-rpc")]
pub mod quinn;

#[allow(async_fn_in_trait)]
pub trait RpcStream {
    async fn read(&mut self) -> Option<Vec<u8>>;
    async fn write(&mut self, buf: &[u8]) -> Option<()>;
    async fn close(&mut self);
}

#[allow(async_fn_in_trait)]
pub trait RpcClient<D, S: RpcStream>: Clone {
    async fn connect(&self, dest: D, server_name: &str) -> Option<S>;
}

#[allow(async_fn_in_trait)]
pub trait RpcServer<S: RpcStream> {
    async fn accept(&mut self) -> Option<(String, S)>;
}

pub fn node_vnet_addr(node_id: u32, port: u16) -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(node_id.into(), port))
}
