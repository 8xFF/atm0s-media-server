use cluster::rpc::webrtc::{WebrtcConnectRequestReceivers, WebrtcConnectRequestSender};
use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, MixMinusSource, MixMinusToggle, ReceiverDisconnect, ReceiverLimit, ReceiverSwitch, RemoteTrackRpcIn, RemoteTrackRpcOut, SenderToggle, RemotePeer},
    EndpointRpcIn, EndpointRpcOut, RpcRequest, RpcResponse,
};
use serde::{Deserialize, Serialize};

#[allow(unused)]
fn request_to_json<R: Serialize>(req_id: u32, request: &str, req: RpcRequest<R>) -> String {
    serde_json::json!({
        "req_id": req_id,
        "type": "request",
        "request": request,
        "data": req.data
    })
    .to_string()
}

fn event_to_json<E: Serialize>(event: &str, e: E) -> String {
    serde_json::json!({
        "type": "event",
        "event": event,
        "data": e
    })
    .to_string()
}

#[derive(Debug, PartialEq, Eq)]
pub enum RpcError {
    InvalidRpc(Option<u64>),
    InvalidJson(Option<u64>),
}

pub fn rpc_to_string(rpc: EndpointRpcOut) -> String {
    match rpc {
        EndpointRpcOut::MixMinusSourceAddRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::MixMinusSourceRemoveRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::MixMinusToggleRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::TrackAdded(res) => event_to_json("stream_added", res),
        EndpointRpcOut::TrackUpdated(res) => event_to_json("stream_updated", res),
        EndpointRpcOut::TrackRemoved(res) => event_to_json("stream_removed", res),
        EndpointRpcOut::SubscribePeerRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::UnsubscribePeerRes(res) => serde_json::to_string(&res).expect("should serialize json"),
    }
}

pub fn rpc_internal_to_string(rpc: TransportRpcOut) -> String {
    match rpc {
        TransportRpcOut::UpdateSdpRes(res) => serde_json::to_string(&res).expect("should serialize json"),
    }
}

pub fn rpc_remote_track_to_string(rpc: RemoteTrackRpcOut) -> String {
    match rpc {
        RemoteTrackRpcOut::ToggleRes(res) => serde_json::to_string(&res).expect("should serialize json"),
    }
}

pub fn rpc_local_track_to_string(rpc: LocalTrackRpcOut) -> String {
    match rpc {
        LocalTrackRpcOut::SwitchRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        LocalTrackRpcOut::LimitRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        LocalTrackRpcOut::DisconnectRes(res) => serde_json::to_string(&res).expect("should serialize json"),
    }
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
pub struct UpdateSdp {
    pub sdp: String,
    pub senders: Vec<WebrtcConnectRequestSender>,
    pub receivers: WebrtcConnectRequestReceivers,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct UpdateSdpResponse {
    pub sdp: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TransportRpcIn {
    UpdateSdp(RpcRequest<UpdateSdp>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum TransportRpcOut {
    UpdateSdpRes(RpcResponse<UpdateSdpResponse>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum IncomingRpc {
    Endpoint(EndpointRpcIn),
    Transport(TransportRpcIn),
    RemoteTrack(String, RemoteTrackRpcIn),
    LocalTrack(String, LocalTrackRpcIn),
}

pub fn rpc_from_string(s: &str) -> Result<IncomingRpc, RpcError> {
    let json: serde_json::Value = serde_json::from_str(s).map_err(|_e| RpcError::InvalidJson(None))?;
    let rpc_type = json["type"].as_str().ok_or(RpcError::InvalidRpc(None))?;
    if rpc_type.eq("request") {
        let req_id = json["req_id"].as_u64().ok_or(RpcError::InvalidRpc(None))?;
        let request = json["request"].as_str().ok_or(RpcError::InvalidRpc(Some(req_id)))?;
        let value = json["data"].clone();
        match request {
            "peer.close" => Ok(IncomingRpc::Endpoint(EndpointRpcIn::PeerClose)),
            "sender.toggle" => match serde_json::from_value::<SenderToggle>(value) {
                Ok(params) => Ok(IncomingRpc::RemoteTrack(params.name.clone(), RemoteTrackRpcIn::Toggle(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson(Some(req_id))),
            },
            "receiver.switch" => match serde_json::from_value::<ReceiverSwitch>(value) {
                Ok(params) => Ok(IncomingRpc::LocalTrack(params.id.clone(), LocalTrackRpcIn::Switch(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson(Some(req_id))),
            },
            "receiver.limit" => match serde_json::from_value::<ReceiverLimit>(value) {
                Ok(params) => Ok(IncomingRpc::LocalTrack(params.id.clone(), LocalTrackRpcIn::Limit(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson(Some(req_id))),
            },
            "receiver.disconnect" => match serde_json::from_value::<ReceiverDisconnect>(value) {
                Ok(params) => Ok(IncomingRpc::LocalTrack(params.id.clone(), LocalTrackRpcIn::Disconnect(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson(Some(req_id))),
            },
            "mix_minus.add" => match serde_json::from_value::<MixMinusSource>(value) {
                Ok(params) => Ok(IncomingRpc::Endpoint(EndpointRpcIn::MixMinusSourceAdd(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson(Some(req_id))),
            },
            "mix_minus.remove" => match serde_json::from_value::<MixMinusSource>(value) {
                Ok(params) => Ok(IncomingRpc::Endpoint(EndpointRpcIn::MixMinusSourceRemove(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson(Some(req_id))),
            },
            "mix_minus.toggle" => match serde_json::from_value::<MixMinusToggle>(value) {
                Ok(params) => Ok(IncomingRpc::Endpoint(EndpointRpcIn::MixMinusToggle(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson(Some(req_id))),
            },
            "peer.updateSdp" => match serde_json::from_value::<UpdateSdp>(value) {
                Ok(params) => Ok(IncomingRpc::Transport(TransportRpcIn::UpdateSdp(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson(Some(req_id))),
            },
            "room.subscribe" => match serde_json::from_value::<RemotePeer>(value) {
                Ok(params) => Ok(IncomingRpc::Endpoint(EndpointRpcIn::SubscribePeer(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson(Some(req_id))),
            },
            "room.unsubscribe" => match serde_json::from_value::<RemotePeer>(value) {
                Ok(params) => Ok(IncomingRpc::Endpoint(EndpointRpcIn::UnsubscribePeer(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson(Some(req_id))),
            },
            _ => Err(RpcError::InvalidRpc(Some(req_id))),
        }
    } else if rpc_type.eq("event") {
        Err(RpcError::InvalidRpc(None))
    } else {
        Err(RpcError::InvalidRpc(None))
    }
}

#[cfg(test)]
mod tests {
    use transport::MediaKind;

    #[test]
    fn from_string_should_work() {
        let s = r#"{"req_id":1,"type":"request","request":"sender.toggle","data":{"name":"test","kind":"audio","label":"label"}}"#;
        let rpc = super::rpc_from_string(s).unwrap();
        match rpc {
            super::IncomingRpc::RemoteTrack(name, super::RemoteTrackRpcIn::Toggle(req)) => {
                assert_eq!(name, "test");
                assert_eq!(req.req_id, 1);
                assert_eq!(req.data.name, "test");
                assert_eq!(req.data.kind, MediaKind::Audio);
                assert_eq!(req.data.label, Some("label".to_string()));
            }
            _ => panic!("should be remote track toggle"),
        }
    }

    #[test]
    fn invalid_request() {
        assert_eq!(super::rpc_from_string(r#"{"req_id":1,"type":"request"}"#), Err(super::RpcError::InvalidRpc(Some(1))));
    }

    #[test]
    fn invalid_event() {
        assert_eq!(super::rpc_from_string(r#"{"aaaa":1}"#), Err(super::RpcError::InvalidRpc(None)));
    }
}
