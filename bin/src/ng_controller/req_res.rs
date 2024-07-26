use media_server_protocol::{
    cluster::gen_cluster_session_id,
    endpoint::{ClusterConnId, PeerId, RoomId},
    transport::{rtpengine, RpcReq, RpcRes},
};
use rtpengine_ngcontrol::{NgCmdResult, NgCommand};

pub fn ng_cmd_to_rpc(cmd: NgCommand) -> Option<RpcReq<ClusterConnId>> {
    match cmd {
        NgCommand::Ping {} => Some(RpcReq::RtpEngine(rtpengine::RpcReq::Ping)),
        NgCommand::Offer { sdp, call_id, from_tag, .. } => {
            let session_id = gen_cluster_session_id();
            Some(RpcReq::RtpEngine(rtpengine::RpcReq::Connect(rtpengine::RtpConnectRequest {
                call_id: RoomId(call_id),
                leg_id: PeerId(from_tag),
                sdp,
                session_id,
            })))
        }
        NgCommand::Answer { sdp, call_id, to_tag, .. } => {
            let session_id = gen_cluster_session_id();
            Some(RpcReq::RtpEngine(rtpengine::RpcReq::Connect(rtpengine::RtpConnectRequest {
                call_id: RoomId(call_id),
                leg_id: PeerId(to_tag),
                sdp,
                session_id,
            })))
        }
        _ => None,
    }
}

pub fn rpc_result_to_ng_res(res: RpcRes<ClusterConnId>) -> Option<NgCmdResult> {
    match res {
        RpcRes::RtpEngine(rtpengine::RpcRes::Ping(Ok(res))) => Some(NgCmdResult::Pong { result: res }),
        RpcRes::RtpEngine(rtpengine::RpcRes::Connect(res)) => match res {
            Ok((_conn, sdp)) => Some(NgCmdResult::Answer {
                result: "ok".to_string(),
                sdp: Some(sdp),
            }),
            Err(e) => Some(NgCmdResult::Error {
                result: "error".to_string(),
                error_reason: e.message.to_string(),
            }),
        },
        _ => None,
    }
}
