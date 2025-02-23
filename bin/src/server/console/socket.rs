use futures::{SinkExt, StreamExt};
use media_server_secure::MediaConsoleSecure;
use poem::{
    get, handler,
    web::{
        websocket::{CloseCode, Message, WebSocket},
        Data,
    },
    EndpointExt, Error, FromRequest, IntoResponse, Request, RequestBody, Result, Route,
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
fn ws(Token(token): Token, ws: WebSocket, ctx: Data<&ConsoleApisCtx>) -> impl IntoResponse {
    let storage = ctx.storage.clone();
    let console_ctx = ctx.clone();
    ws.on_upgrade(move |mut socket| async move {
        let (mut sink, mut stream) = socket.split();
        if !console_ctx.secure.validate_token(&token) {
            log::error!("Invalid token: {token:?}");
            sink.send(Message::Close(Some((CloseCode::Invalid, "Invalid token".to_string())))).await.ok();
            sink.close().await.ok();
            drop(sink);
            drop(stream);
            return;
        }
        let snapshot = storage.network_node();
        let event = NetworkNodeEvent::Snapshot(snapshot);
        let event = serde_json::to_string(&event).expect("must convert event to json");

        if let Err(err) = sink.send(Message::Text(event)).await {
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
}

pub fn console_websocket_handle(ctx: ConsoleApisCtx) -> Route {
    Route::new().nest("/network", get(ws.data(ctx)))
}
