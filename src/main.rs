use async_nats::connect;
use axum::{
    extract::{Path, State, WebSocketUpgrade},
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use futures::{stream::StreamExt, SinkExt};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

// Embed the HTML file at compile time
const INDEX_HTML: &str = include_str!("../static/index.html");

// Maximum number of messages to buffer for new clients
const CHANNEL_CAPACITY: usize = 100;

// Message type to include subject for filtering
#[derive(Clone, Debug)]
struct NatsMessage {
    subject: String,
    payload: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up broadcast channel for sharing NATS messages between clients
    let (tx, _) = broadcast::channel::<NatsMessage>(CHANNEL_CAPACITY);
    let tx = Arc::new(tx);

    // Connect to NATS
    let nats_url =
        std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());
    let client = connect(&nats_url).await?;
    println!("Connected to NATS at {}", nats_url);

    // Start NATS subscription handler
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        let mut subscriber = client.subscribe(">").await.unwrap();
        while let Some(msg) = subscriber.next().await {
            let message = NatsMessage {
                subject: msg.subject.clone().to_string(),
                payload: String::from_utf8_lossy(&msg.payload).to_string(),
            };
            let _ = tx_clone.send(message);
        }
    });

    // Set up web server
    let app = Router::new()
        .route("/ws/:filter", get(ws_handler))
        .route("/", get(index_handler));

    let app = app.with_state(tx);

    // Create the listener
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    println!("Starting web server on port 3000");

    // Start serving
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index_handler() -> impl IntoResponse {
    Html(INDEX_HTML)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(tx): State<Arc<broadcast::Sender<NatsMessage>>>,
    Path(filter): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| websocket(socket, tx, filter))
}

async fn websocket(
    stream: axum::extract::ws::WebSocket,
    tx: Arc<broadcast::Sender<NatsMessage>>,
    filter: String,
) {
    let (mut sender, _receiver) = stream.split();
    let mut rx = tx.subscribe();

    while let Ok(msg) = rx.recv().await {
        // Apply filter
        if filter == ">" || matches_filter(&filter, &msg.subject) {
            let message = format!("Subject: {}, Payload: {}", msg.subject, msg.payload);
            if sender
                .send(axum::extract::ws::Message::Text(message))
                .await
                .is_err()
            {
                break;
            }
        }
    }
}

// Simple NATS subject filter matching
fn matches_filter(filter: &str, subject: &str) -> bool {
    let filter_parts: Vec<&str> = filter.split('.').collect();
    let subject_parts: Vec<&str> = subject.split('.').collect();

    if filter_parts.len() > subject_parts.len() {
        return false;
    }

    for (f, s) in filter_parts.iter().zip(subject_parts.iter()) {
        if f != &"*" && f != &">" && f != s {
            return false;
        }
        if f == &">" {
            return true;
        }
    }

    filter_parts.len() == subject_parts.len()
}
