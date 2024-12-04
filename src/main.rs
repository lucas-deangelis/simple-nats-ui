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

const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>NATS Web UI</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            margin: 0;
            padding: 20px;
            height: 100vh;
            overflow-x: auto;
        }
        .columns-container {
            display: flex;
            gap: 20px;
            height: calc(100vh - 40px);
            min-width: min-content;
        }
        .column {
            width: 300px;
            flex-shrink: 0;
            display: flex;
            flex-direction: column;
            border: 1px solid #ccc;
            border-radius: 4px;
        }
        .column-header {
            padding: 10px;
            background: #f5f5f5;
            border-bottom: 1px solid #ccc;
            display: flex;
            gap: 10px;
            align-items: center;
        }
        .filter-display {
            flex-grow: 1;
            font-family: monospace;
            padding: 5px 10px;
            background: #fff;
            border: 1px solid #ddd;
            border-radius: 3px;
        }
        .messages {
            flex-grow: 1;
            overflow-y: auto;
            padding: 10px;
        }
        .message {
            padding: 5px 0;
            border-bottom: 1px solid #eee;
            word-break: break-word;
        }
        button {
            padding: 5px 10px;
            cursor: pointer;
        }
        .add-column {
            position: sticky;
            left: 20px;
            top: 20px;
            z-index: 1000;
            background: white;
            border: 2px dashed #ccc;
            padding: 10px;
            border-radius: 4px;
        }
        #new-filter {
            width: 200px;
            padding: 5px;
            margin-right: 10px;
        }
    </style>
</head>
<body>
    <div class="add-column">
        <input type="text" id="new-filter" placeholder="Enter NATS subject filter">
        <button onclick="addColumn()">Add Column</button>
    </div>
    
    <div class="columns-container" id="columns">
        <!-- First column with ">" filter -->
        <div class="column" data-filter=">">
            <div class="column-header">
                <span class="filter-display">&gt;</span>
                <!-- No delete button for the first column -->
            </div>
            <div class="messages"></div>
        </div>
    </div>

    <script>
        const columns = new Map(); // Store WebSocket connections by filter

        function addColumn(filter = "") {
            if (!filter && filter !== ">") {
                filter = document.getElementById("new-filter").value.trim();
                if (!filter) return;
                document.getElementById("new-filter").value = "";
            }

            // Check if column already exists
            if (columns.has(filter)) {
                alert("This filter already exists!");
                return;
            }

            const column = document.createElement("div");
            column.className = "column";
            column.dataset.filter = filter;

            const header = document.createElement("div");
            header.className = "column-header";
            
            const filterDisplay = document.createElement("span");
            filterDisplay.className = "filter-display";
            filterDisplay.textContent = filter;
            header.appendChild(filterDisplay);

            if (filter !== ">") {
                const deleteBtn = document.createElement("button");
                deleteBtn.textContent = "Delete";
                deleteBtn.onclick = () => {
                    const ws = columns.get(filter);
                    if (ws) {
                        ws.close();
                        columns.delete(filter);
                    }
                    column.remove();
                };
                header.appendChild(deleteBtn);
            }

            const messages = document.createElement("div");
            messages.className = "messages";

            column.appendChild(header);
            column.appendChild(messages);

            // Add new columns to the right
            document.getElementById("columns").appendChild(column);

            // Connect WebSocket
            connectWebSocket(filter, messages);
        }

        function connectWebSocket(filter, messagesDiv) {
            const wsProtocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
            const ws = new WebSocket(`${wsProtocol}//${window.location.host}/ws/${encodeURIComponent(filter)}`);
            columns.set(filter, ws);

            ws.onmessage = (event) => {
                const messageDiv = document.createElement("div");
                messageDiv.className = "message";
                messageDiv.textContent = event.data;
                messagesDiv.appendChild(messageDiv);
                
                // Auto-scroll to bottom
                messagesDiv.scrollTop = messagesDiv.scrollHeight;

                // Keep only last 1000 messages
                while (messagesDiv.children.length > 1000) {
                    messagesDiv.removeChild(messagesDiv.firstChild);
                }
            };

            ws.onclose = () => {
                const messageDiv = document.createElement("div");
                messageDiv.className = "message";
                messageDiv.style.color = "red";
                messageDiv.textContent = "WebSocket connection closed. Refresh to reconnect.";
                messagesDiv.appendChild(messageDiv);
            };
        }

        // Initialize the first column with ">"
        connectWebSocket(">", document.querySelector(".messages"));
    </script>
</body>
</html>"#;

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
