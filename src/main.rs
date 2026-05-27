use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::{mpsc, Mutex};
use tower_http::services::ServeDir;
use tracing::{error, info};
use uuid::Uuid;

type Tx = mpsc::UnboundedSender<Message>;
type Clients = Arc<Mutex<HashMap<String, Tx>>>;

#[derive(Clone)]
struct AppState {
    clients: Clients,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ChatPayload {
    #[serde(rename = "type")]
    message_type: String,
    sender: String,
    text: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("rust_ws_chat=debug,tower_http=debug,axum=info")
        .init();

    let state = AppState {
        clients: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .nest_service("/", ServeDir::new("static"))
        .with_state(state);

    let addr: SocketAddr = "0.0.0.0:8080".parse().expect("invalid bind address");
    info!("server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind listener");
    axum::serve(listener, app).await.expect("server error");
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let client_id = Uuid::new_v4().to_string();
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    {
        let mut clients = state.clients.lock().await;
        clients.insert(client_id.clone(), tx);
    }
    info!("client connected: {client_id}");

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    while let Some(result) = ws_receiver.next().await {
        match result {
            Ok(Message::Text(raw)) => {
                if let Some(chat) = parse_and_sanitize(&raw) {
                    broadcast(&state, &client_id, &chat).await;
                }
            }
            Ok(Message::Close(_)) => {
                break;
            }
            Ok(_) => {}
            Err(err) => {
                error!("ws receive error ({client_id}): {err}");
                break;
            }
        }
    }

    {
        let mut clients = state.clients.lock().await;
        clients.remove(&client_id);
    }
    send_task.abort();
    info!("client disconnected: {client_id}");
}

fn parse_and_sanitize(raw: &str) -> Option<ChatPayload> {
    let mut payload: ChatPayload = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return None,
    };

    if payload.message_type != "message" {
        return None;
    }

    payload.sender = sanitize_text(&payload.sender, 32);
    payload.text = sanitize_text(&payload.text, 500);

    if payload.sender.is_empty() || payload.text.is_empty() {
        return None;
    }
    Some(payload)
}

fn sanitize_text(input: &str, max_len: usize) -> String {
    let trimmed = input.trim();
    let clipped: String = trimmed.chars().take(max_len).collect();
    clipped
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

async fn broadcast(state: &AppState, sender_id: &str, payload: &ChatPayload) {
    let encoded = match serde_json::to_string(payload) {
        Ok(v) => v,
        Err(err) => {
            error!("serialize error: {err}");
            return;
        }
    };

    let clients = state.clients.lock().await;
    for (client_id, tx) in clients.iter() {
        if client_id == sender_id {
            continue;
        }
        let _ = tx.send(Message::Text(encoded.clone()));
    }
}
