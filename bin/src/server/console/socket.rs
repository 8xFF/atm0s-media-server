use futures::{SinkExt, StreamExt};
use media_server_secure::MediaConsoleSecure;
use poem::{
    get, handler,
    web::{
        websocket::{Message, WebSocket},
        Data,
    },
    EndpointExt, Error, FromRequest, IntoResponse, Request, RequestBody, Response, Result, Route,
};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tokio::select;

use crate::{http::api_console::ConsoleApisCtx, server::console_storage::NetworkNodeEvent};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct QueryParams {
    token: String,
}

struct Token(String);

impl<'a> FromRequest<'a> for Token {
    async fn from_request(req: &'a Request, _body: &mut RequestBody) -> Result<Self> {
        let token = req.params::<QueryParams>().map_err(|_| Error::from_string("missing token", StatusCode::BAD_REQUEST))?.token;
        Ok(Token(token.to_string()))
    }
}

#[handler]
fn network_visualization_ws(Token(token): Token, ws: WebSocket, ctx: Data<&ConsoleApisCtx>) -> Response {
    log::info!("Network visualization websocket connected: {token:?}");
    if !ctx.secure.validate_token(&token) {
        log::error!("Invalid token: {token:?}");
        return Response::builder().status(StatusCode::UNAUTHORIZED).body("Unauthorized");
    }
    let storage = ctx.storage.clone();
    ws.on_upgrade(move |mut socket| async move {
        let (mut sink, mut stream) = socket.split();
        let nodes = storage.network_nodes();

        log::info!("Send snapshot: {} nodes", nodes.len());
        let event = NetworkNodeEvent::Snapshot(nodes);
        if let Err(err) = sink.send(Message::Text(serde_json::to_string(&event).expect("must convert event to json"))).await {
            log::error!("Failed to send snapshot: {}", err);
            return;
        }

        let mut receiver = storage.subscribe();
        drop(storage);
        loop {
            select! {
                event = receiver.recv() => match event {
                    Ok(ev) => {
                        if let Err(err) = sink.send(Message::Text(serde_json::to_string(&ev).expect("must convert event to json"))).await {
                            log::error!("Failed to send event: {}", err);
                            break;
                        }
                    }
                    Err(err) => {
                        log::error!("Failed to receive event: {}", err);
                        break;
                    }
                },
                event = stream.next() => match event {
                    Some(_) => {}
                    None => {
                        break;
                    }
                }
            }
        }
    })
    .into_response()
}

pub fn console_websocket_handle(ctx: ConsoleApisCtx) -> Route {
    Route::new().at("/network", get(network_visualization_ws.data(ctx)))
}
