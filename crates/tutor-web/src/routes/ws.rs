use std::sync::Arc;

use axum::{
    Router,
    extract::ws::{Message, WebSocket},
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use futures::{SinkExt, StreamExt};

use crate::session::SessionPool;

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(pool): State<Arc<SessionPool>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, pool, session_id))
}

async fn handle_socket(socket: WebSocket, pool: Arc<SessionPool>, session_id: String) {
    let entry = match pool.get(&session_id) {
        Some(e) => e,
        None => return,
    };
    // Take the receiver stored during session creation
    let mut event_rx = match pool.take_rx(&session_id) {
        Some(rx) => rx,
        None => return,
    };

    let (mut ws_sink, mut ws_stream) = socket.split();

    // Forward events from the agent harness to the WebSocket client
    let send_task = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let json = match serde_json::to_string(&event) {
                Ok(j) => j,
                Err(_) => continue,
            };
            if ws_sink.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Receive messages from the client
    while let Some(Ok(msg)) = ws_stream.next().await {
        match msg {
            Message::Text(ref _text) => {
                // TODO: wire to CapabilityRouter.run() with streaming
                let _ = entry.stream.content("Processing...", false).await;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    send_task.abort();
}

pub fn ws_router(pool: Arc<SessionPool>) -> Router {
    Router::new()
        .route("/ws/sessions/:id", get(ws_handler))
        .with_state(pool)
}
