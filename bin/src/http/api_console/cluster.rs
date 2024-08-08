use crate::server::console_storage::{ConsoleNode, Zone, ZoneDetails};

use super::{super::Response, ConsoleApisCtx, ConsoleAuthorization};
use media_server_protocol::cluster::ZoneId;
use poem::web::Data;
use poem_openapi::{param::Path, payload::Json, OpenApi};

pub struct Apis;

#[OpenApi]
impl Apis {
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
