use std::sync::Arc;

use clap::Parser;
use cluster::{atm0s_sdn::SystemTimer, SessionTokenSigner};
use metrics_dashboard::build_dashboard_route;
use poem::Route;
use poem_openapi::OpenApiService;

use crate::rpc::http::HttpRpcServer;

use self::http::{HttpContext, TokenGenerateHttpApis};

mod http;

/// Media Server Webrtc
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct TokenGenerateArgs {}

pub async fn run_token_generate_server(http_port: u16, http_tls: bool, _opts: TokenGenerateArgs, secret: &str, token_signer: Arc<dyn SessionTokenSigner + Send + Sync>) -> Result<(), &'static str> {
    let mut http_server: HttpRpcServer<()> = crate::rpc::http::HttpRpcServer::new(http_port, http_tls);
    let api_service = OpenApiService::new(TokenGenerateHttpApis, "Token Generate Server", "1.0.0").server("/");
    let ui = api_service.swagger_ui();
    let spec = api_service.spec();

    let route = Route::new()
        .nest("/", api_service)
        .nest("/dashboard/", build_dashboard_route())
        .nest("/ui/", ui)
        .at("/spec/", poem::endpoint::make_sync(move |_| spec.clone()));

    http_server
        .start(
            route,
            HttpContext {
                timer: Arc::new(SystemTimer()),
                secret: secret.to_string(),
                signer: token_signer,
            },
        )
        .await;

    while let Some(_) = http_server.recv().await {}

    Ok(())
}
