use futures::{SinkExt, StreamExt};
use poem::{
    get, handler,
    web::{
        websocket::{Message, WebSocket},
        Data,
    },
    EndpointExt, IntoResponse, Route,
};
use tokio::select;

use crate::server::console_storage::NetworkNodeEvent;

use super::storage::StorageShared;

#[handler]
fn ws(ws: WebSocket, storage: Data<&StorageShared>) -> impl IntoResponse {
    let storage = storage.clone();
    ws.on_upgrade(move |socket| async move {
        let (mut sink, mut stream) = socket.split();
        let snapshot = storage.network_node();
        let event = NetworkNodeEvent::Snapshot(snapshot);
        let event = serde_json::to_string(&event).expect("must convert event to json");

        if let Err(err) = sink.send(Message::Text(event)).await {
            log::error!("Failed to send snapshot: {}", err);
            return;
        }

        let mut receiver = storage.subcribe();
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

pub fn console_websocket_handle(storage: StorageShared) -> Route {
    Route::new().nest("/network", get(ws.data(storage.clone())))
}
