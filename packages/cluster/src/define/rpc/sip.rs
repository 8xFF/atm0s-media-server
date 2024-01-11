pub struct SipInviteRequest {
    room_id: String,
    callee: String,
    server_alias: Option<String>,
}

pub struct SipInviteResponse {
    call_id: String,
    status: String,
}
