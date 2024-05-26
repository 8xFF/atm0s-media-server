#[cfg(feature = "quinn-rpc")]
pub mod quinn;

#[allow(async_fn_in_trait)]
pub trait RpcStream {
    async fn read(&mut self) -> Option<Vec<u8>>;
    async fn write(&mut self, buf: &[u8]) -> Option<()>;
}

#[allow(async_fn_in_trait)]
pub trait RpcClient<D, S: RpcStream> {
    async fn connect(&mut self, dest: D, server_name: &str) -> Option<S>;
}

#[allow(async_fn_in_trait)]
pub trait RpcServer<S: RpcStream> {
    async fn accept(&mut self) -> Option<(String, S)>;
}
