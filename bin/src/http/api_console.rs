use std::net::SocketAddr;

use media_server_protocol::{
    protobuf::cluster_connector::MediaConnectorServiceClient,
    rpc::quinn::{QuinnClient, QuinnStream},
};
use media_server_secure::{jwt::MediaConsoleSecureJwt, MediaConsoleSecure};
use poem::Request;
use poem_openapi::{auth::ApiKey, SecurityScheme};

use crate::server::console_storage::StorageShared;

pub mod cluster;
pub mod connector;
pub mod user;

#[derive(Clone)]
pub struct ConsoleApisCtx {
    pub secure: MediaConsoleSecureJwt, //TODO make it generic
    pub storage: StorageShared,
    pub connector: MediaConnectorServiceClient<SocketAddr, QuinnClient, QuinnStream>,
}

/// ApiKey authorization
#[derive(SecurityScheme)]
#[oai(ty = "api_key", key_name = "X-API-Key", key_in = "header", checker = "api_checker")]
struct ConsoleAuthorization(());

async fn api_checker(req: &Request, api_key: ApiKey) -> Option<()> {
    let data = req.data::<ConsoleApisCtx>()?;
    data.secure.validate_token(&api_key.key).then(|| ())
}
