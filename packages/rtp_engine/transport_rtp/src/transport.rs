use media_server_protocol::transport::RpcResult;

pub enum RtpExtIn {
    Ping(u64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum RtpExtOut {
    // req_id, result
    Pong(u64, RpcResult<String>),
}
