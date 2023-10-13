use endpoint::{
    rpc::{MixMinusSource, MixMinusToggle, ReceiverDisconnect, ReceiverLimit, ReceiverSwitch, SenderToggle},
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

pub enum RpcError {
    InvalidRpc,
    InvalidJson,
}

pub fn rpc_to_string(rpc: EndpointRpcOut) -> String {
    match rpc {
        EndpointRpcOut::SenderToggleRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::ReceiverSwitchRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::ReceiverLimitRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::ReceiverDisconnectRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::MixMinusSourceAddRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::MixMinusSourceRemoveRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::MixMinusToggleRes(res) => serde_json::to_string(&res).expect("should serialize json"),
        EndpointRpcOut::TrackAdded(res) => event_to_json("stream_added", res),
        EndpointRpcOut::TrackUpdated(res) => event_to_json("stream_updated", res),
        EndpointRpcOut::TrackRemoved(res) => event_to_json("stream_removed", res),
    }
}

pub fn rpc_from_string(s: &str) -> Result<EndpointRpcIn, RpcError> {
    let json: serde_json::Value = serde_json::from_str(s).map_err(|e| RpcError::InvalidJson)?;
    let rpc_type = json["type"].as_str().ok_or(RpcError::InvalidRpc)?;
    if rpc_type.eq("request") {
        let req_id = json["req_id"].as_u64().ok_or(RpcError::InvalidRpc)?;
        let request = json["request"].as_str().ok_or(RpcError::InvalidRpc)?;
        let value = json["data"].clone();
        match request {
            "peer.close" => Ok(EndpointRpcIn::PeerClose),
            "sender.toggle" => match serde_json::from_value::<SenderToggle>(value) {
                Ok(params) => Ok(EndpointRpcIn::SenderToggle(RpcRequest::from(req_id, params))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            "receiver.switch" => match serde_json::from_value::<ReceiverSwitch>(value) {
                Ok(params) => Ok(EndpointRpcIn::ReceiverSwitch(RpcRequest::from(req_id, params))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            "receiver.limit" => match serde_json::from_value::<ReceiverLimit>(value) {
                Ok(params) => Ok(EndpointRpcIn::ReceiverLimit(RpcRequest::from(req_id, params))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            "receiver.disconnect" => match serde_json::from_value::<ReceiverDisconnect>(value) {
                Ok(params) => Ok(EndpointRpcIn::ReceiverDisconnect(RpcRequest::from(req_id, params))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            "mix_minus.add" => match serde_json::from_value::<MixMinusSource>(value) {
                Ok(params) => Ok(EndpointRpcIn::MixMinusSourceAdd(RpcRequest::from(req_id, params))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            "mix_minus.remove" => match serde_json::from_value::<MixMinusSource>(value) {
                Ok(params) => Ok(EndpointRpcIn::MixMinusSourceRemove(RpcRequest::from(req_id, params))),
                Err(_err) => Err(RpcError::InvalidJson),
            },
            "mix_minus.toggle" => match serde_json::from_value::<MixMinusToggle>(value) {
                Ok(params) => Ok(EndpointRpcIn::MixMinusToggle(RpcRequest::from(req_id, params))),
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
