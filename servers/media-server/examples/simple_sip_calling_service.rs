use cluster::rpc::sip::{SipIncomingInviteRequest, SipIncomingInviteResponse, SipIncomingRegisterRequest, SipIncomingRegisterResponse};
use poem::{handler, listener::TcpListener, post, web::Json, Route, Server};

#[handler]
fn hook_register(req: Json<SipIncomingRegisterRequest>) -> Json<SipIncomingRegisterResponse> {
    log::info!("hook_register: {:?}", req);
    let password = &req.0.username;
    let ha1 = md5::compute(format!("{}:{}:{}", req.0.username, req.0.realm, password));

    Json(SipIncomingRegisterResponse {
        success: true,
        ha1: Some(format!("{:x}", ha1)),
    })
}

#[handler]
fn hook_invite(req: Json<SipIncomingInviteRequest>) -> Json<SipIncomingInviteResponse> {
    log::info!("hook_invite: {:?}", req);
    Json(SipIncomingInviteResponse {
        room_id: Some("demo".to_string()),
        strategy: cluster::rpc::sip::SipIncomingInviteStrategy::Accept,
    })
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info");
    }
    tracing_subscriber::fmt::init();

    let app = Route::new().at("/hooks/register", post(hook_register)).at("/hooks/invite", post(hook_invite));

    Server::new(TcpListener::bind("0.0.0.0:3000")).run(app).await
}
