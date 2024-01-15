use async_std::channel::Sender;
use cluster::rpc::general::MediaEndpointCloseRequest;
use cluster::rpc::general::MediaEndpointCloseResponse;
use media_utils::Response;
use poem::{
    http::StatusCode,
    web::{Data, Path},
    Result,
};
use poem_openapi::{payload::Json, OpenApi};

use crate::rpc::http::RpcReqResHttp;
use crate::server::MediaServerContext;

use super::RpcEvent;

pub struct RtmpHttpApis;

#[OpenApi]
impl RtmpHttpApis {
    /// delete Rtmp conn
    #[oai(path = "/rtmp/conn/:conn_id", method = "delete")]
    async fn conn_rtmp_delete(&self, Data(ctx): Data<&(Sender<RpcEvent>, MediaServerContext<()>)>, conn_id: Path<String>) -> Result<Json<Response<String>>> {
        log::info!("[HttpApis] close Rtmp endpoint conn {}", conn_id.0);
        let (req, rx) = RpcReqResHttp::<MediaEndpointCloseRequest, MediaEndpointCloseResponse>::new(MediaEndpointCloseRequest { conn_id: conn_id.0.clone() });
        ctx.0
            .send(RpcEvent::MediaEndpointClose(Box::new(req)))
            .await
            .map_err(|_e| poem::Error::from_status(StatusCode::INTERNAL_SERVER_ERROR))?;
        let res = rx.recv().await.map_err(|e| poem::Error::new(e, StatusCode::INTERNAL_SERVER_ERROR))?;
        let _res = res.map_err(|_e| poem::Error::from_status(StatusCode::BAD_REQUEST))?;
        log::info!("[HttpApis] Rtmp endpoint closed conn {}", conn_id.0);
        Ok(Json(Response {
            status: true,
            error: None,
            data: Some("OK".to_string()),
        }))
    }
}
