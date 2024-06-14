use super::{super::Response, ConsoleApisCtx};
use poem::web::Data;
use poem_openapi::{payload::Json, OpenApi};
use std::net::IpAddr;

use atm0s_sdn::NodeId;

#[derive(poem_openapi::Object)]
pub struct MediaNode {
    pub ip: IpAddr,
    pub node_id: NodeId,
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
    pub live: u64,
    pub max: u64,
}

#[derive(poem_openapi::Object)]
pub struct GatewayNode {
    pub ip: IpAddr,
    pub node_id: NodeId,
    pub cpu: u8,
    pub memory: u8,
    pub disk: u8,
    pub live: u64,
    pub max: u64,
    pub nodes: Vec<MediaNode>,
}

pub struct Apis;

#[OpenApi]
impl Apis {
    /// get gateways
    #[oai(path = "/gateways", method = "get")]
    async fn gateways(&self, Data(ctx): Data<&ConsoleApisCtx>) -> Json<Response<Vec<GatewayNode>>> {
        Json(Response {
            status: true,
            error: None,
            data: Some(vec![]),
        })
    }
}
