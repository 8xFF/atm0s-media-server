use crate::server::console_storage::{ConsoleNode, Zone, ZoneDetails};

use super::{super::Response, ConsoleApisCtx, ConsoleAuthorization};
use media_server_protocol::cluster::ZoneId;
use poem::web::Data;
use poem_openapi::{
    param::{Path, Query},
    payload::Json,
    Enum, OpenApi,
};

pub struct Apis;

#[derive(Debug, Clone, Enum)]
enum NodeType {
    Console,
    Gateway,
    Connector,
    Media,
}

#[OpenApi]
impl Apis {
    /// Get seed nodes for a zone and node type
    /// With console node type, it will return all consoles.
    /// With gateway node type, it will return all consoles and gateways in the same zone.
    /// With connector node type, it will return all gateways in the zone.
    /// With media node type, it will return all gateways in the zone.
    #[oai(path = "/seeds", method = "get")]
    async fn seeds_for(&self, zone_id: Query<u32>, node_type: Query<NodeType>, Data(ctx): Data<&ConsoleApisCtx>) -> Json<Vec<String>> {
        log::info!("seeds_for zone_id: {}, node_type: {:?}", zone_id.0, node_type.0);
        match node_type.0 {
            NodeType::Console => Json(ctx.storage.consoles().iter().map(|node| node.addr.clone()).collect()),
            NodeType::Gateway => {
                let consoles = ctx.storage.consoles().into_iter().map(|node| node.addr.clone());
                let zone = ctx.storage.zone(ZoneId(zone_id.0));
                let same_zone_gateways = zone.iter().flat_map(|n| n.gateways.iter()).map(|n| n.addr.clone());
                Json(consoles.chain(same_zone_gateways).collect())
            }
            NodeType::Connector => Json(ctx.storage.zone(ZoneId(zone_id.0)).unwrap().gateways.iter().map(|node| node.addr.to_string()).collect()),
            NodeType::Media => Json(ctx.storage.zone(ZoneId(zone_id.0)).unwrap().gateways.iter().map(|node| node.addr.to_string()).collect()),
        }
    }

    /// get consoles from all zones
    #[oai(path = "/consoles", method = "get")]
    async fn consoles(&self, _auth: ConsoleAuthorization, Data(ctx): Data<&ConsoleApisCtx>) -> Json<Response<Vec<ConsoleNode>>> {
        Json(Response {
            status: true,
            data: Some(ctx.storage.consoles()),
            ..Default::default()
        })
    }

    /// get zones
    #[oai(path = "/zones", method = "get")]
    async fn zones(&self, _auth: ConsoleAuthorization, Data(ctx): Data<&ConsoleApisCtx>) -> Json<Response<Vec<Zone>>> {
        Json(Response {
            status: true,
            data: Some(ctx.storage.zones()),
            ..Default::default()
        })
    }

    /// get zone
    #[oai(path = "/zones/:zone_id", method = "get")]
    async fn zone(&self, _auth: ConsoleAuthorization, zone_id: Path<u32>, Data(ctx): Data<&ConsoleApisCtx>) -> Json<Response<ZoneDetails>> {
        if let Some(zone) = ctx.storage.zone(ZoneId(zone_id.0)) {
            Json(Response {
                status: true,
                data: Some(zone),
                ..Default::default()
            })
        } else {
            Json(Response {
                status: false,
                error: Some("ZONE_NOT_FOUND".to_string()),
                ..Default::default()
            })
        }
    }
}
