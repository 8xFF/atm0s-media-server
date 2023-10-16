use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, MixMinusSource, MixMinusToggle, ReceiverDisconnect, ReceiverLimit, ReceiverSwitch, RemoteTrackRpcIn, RemoteTrackRpcOut, SenderToggle},
    EndpointRpcIn, EndpointRpcOut, RpcRequest,
};
use serde::Serialize;

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

#[derive(Debug)]
pub enum RpcError {
    InvalidRpc,
    InvalidJson,
}

pub fn rpc_to_string(rpc: EndpointRpcOut) -> String {
    match rpc {
        EndpointRpcOut::MixMinusSourceAddRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::MixMinusSourceRemoveRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::MixMinusToggleRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::TrackAdded(res) => event_to_json("stream_added", res),
        EndpointRpcOut::TrackUpdated(res) => event_to_json("stream_updated", res),
        EndpointRpcOut::TrackRemoved(res) => event_to_json("stream_removed", res),
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

pub enum IncomingRpc {
    Endpoint(EndpointRpcIn),
    RemoteTrack(String, RemoteTrackRpcIn),
    LocalTrack(String, LocalTrackRpcIn),
}

pub fn rpc_from_string(s: &str) -> Result<IncomingRpc, RpcError> {
    let json: serde_json::Value = serde_json::from_str(s).map_err(|_e| RpcError::InvalidJson)?;
    let rpc_type = json["type"].as_str().ok_or(RpcError::InvalidRpc)?;
    if rpc_type.eq("request") {
        let req_id = json["req_id"].as_u64().ok_or(RpcError::InvalidRpc)?;
        let request = json["request"].as_str().ok_or(RpcError::InvalidRpc)?;
        let value = json["data"].clone();
        match request {
            "peer.close" => Ok(IncomingRpc::Endpoint(EndpointRpcIn::PeerClose)),
            "sender.toggle" => match serde_json::from_value::<SenderToggle>(value) {
                Ok(params) => Ok(IncomingRpc::RemoteTrack(params.name.clone(), RemoteTrackRpcIn::Toggle(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            "receiver.switch" => match serde_json::from_value::<ReceiverSwitch>(value) {
                Ok(params) => Ok(IncomingRpc::LocalTrack(params.id.clone(), LocalTrackRpcIn::Switch(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            "receiver.limit" => match serde_json::from_value::<ReceiverLimit>(value) {
                Ok(params) => Ok(IncomingRpc::LocalTrack(params.id.clone(), LocalTrackRpcIn::Limit(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            "receiver.disconnect" => match serde_json::from_value::<ReceiverDisconnect>(value) {
                Ok(params) => Ok(IncomingRpc::LocalTrack(params.id.clone(), LocalTrackRpcIn::Disconnect(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            "mix_minus.add" => match serde_json::from_value::<MixMinusSource>(value) {
                Ok(params) => Ok(IncomingRpc::Endpoint(EndpointRpcIn::MixMinusSourceAdd(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            "mix_minus.remove" => match serde_json::from_value::<MixMinusSource>(value) {
                Ok(params) => Ok(IncomingRpc::Endpoint(EndpointRpcIn::MixMinusSourceRemove(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            "mix_minus.toggle" => match serde_json::from_value::<MixMinusToggle>(value) {
                Ok(params) => Ok(IncomingRpc::Endpoint(EndpointRpcIn::MixMinusToggle(RpcRequest::from(req_id, params)))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            _ => Err(RpcError::InvalidRpc),
        }
    } else if rpc_type.eq("event") {
        Err(RpcError::InvalidRpc)
    } else {
        Err(RpcError::InvalidRpc)
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
}
