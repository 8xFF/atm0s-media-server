use clap::Parser;
use cluster::{
    rpc::{RpcEmitter, RpcEndpoint, RpcRequest},
    Cluster, ClusterEndpoint,
};
use std::net::SocketAddr;

use super::MediaServerContext;

mod server_udp;
mod sip_in_session;
mod sip_out_session;

pub enum InternalControl {}

/// RTMP Media Server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct SipArgs {
    /// Sip listen addr
    #[arg(env, long)]
    addr: SocketAddr,
}

pub async fn run_sip_server<C, CR, RPC, REQ, EMITTER>(http_port: u16, opts: SipArgs, ctx: MediaServerContext<InternalControl>, mut cluster: C, rpc_endpoint: RPC) -> Result<(), &'static str>
where
    C: Cluster<CR> + Send + 'static,
    CR: ClusterEndpoint + Send + 'static,
    RPC: RpcEndpoint<REQ, EMITTER>,
    REQ: RpcRequest + Send + 'static,
    EMITTER: RpcEmitter + Send + 'static,
{
    server_udp::start_server(cluster, opts.addr).await;
    Ok(())
}
