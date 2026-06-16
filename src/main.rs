use axum::{
    Json, Router,
    extract::{
        Path, Query, State, WebSocketUpgrade,
        ws::{Message as AxumMessage, Utf8Bytes, WebSocket},
    },
    http::Uri,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use rustls_acme::{AcmeConfig, caches::DirCache};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tera::Context;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, info, warn};

mod database;
mod message;
mod room;
mod user;

use database::{create_user, get_message_history, get_rooms, init_db, insert_message};
use message::{MessagePayload, NewMessage};
use room::RoomId;
use user::{User, UserId, UserRole};

type Timestamp = i64;
type Tx = UnboundedSender<Result<AxumMessage, axum::Error>>;
type RoomTxMap = Arc<DashMap<RoomId, Vec<Tx>>>;

#[derive(Clone)]
struct AppState {
    pool: sqlx::SqlitePool,
    room_txs: RoomTxMap,
    templates: tera::Tera,
}

#[derive(Debug, serde::Deserialize)]
struct WsQuery {
    user_id: UserId,
}

// ----- HTTP Handlers -----
async fn index_handler(State(state): State<AppState>) -> Html<String> {
    Html(
        state
            .templates
            .render("index.html", &Context::new())
            .unwrap(),
    )
}

async fn room_list_handler(State(state): State<AppState>) -> Html<String> {
    Html(
        state
            .templates
            .render("room_list.html", &Context::new())
            .unwrap(),
    )
}

async fn room_handler(Path(room_id): Path<RoomId>, State(state): State<AppState>) -> Html<String> {
    let mut ctx = Context::new();
    ctx.insert("room_id", &room_id);
    Html(state.templates.render("room.html", &ctx).unwrap())
}

async fn create_user_handler(
    State(state): State<AppState>,
    Json(payload): Json<User>,
) -> impl IntoResponse {
    match create_user(&state.pool, payload.name().to_string(), *payload.role()).await {
        Ok(user) => Json(user).into_response(),
        Err(e) => {
            error!("Failed to create user : {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "User creation failed",
            )
                .into_response()
        }
    }
}

async fn get_rooms_handler(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let role = params
        .get("role")
        .map(|s| UserRole::from_str(s).unwrap_or_else(|_| UserRole::Parent))
        .unwrap_or_else(|| UserRole::Parent);
    match get_rooms(&state.pool, &role).await {
        Ok(rooms) => Json(rooms).into_response(),
        Err(e) => {
            error!("Failed to get rooms list: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not load rooms",
            )
                .into_response()
        }
    }
}

async fn history_handler(
    State(state): State<AppState>,
    Path(room_id): Path<RoomId>,
) -> impl IntoResponse {
    match get_message_history(&state.pool, room_id).await {
        Ok(msgs) => Json(msgs).into_response(),
        Err(e) => {
            error!("Failed to get message history: {e}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Could not load message history",
            )
                .into_response()
        }
    }
}

// ----- WebSocket Handler -----
async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(room_id): Path<RoomId>,
    Query(query): Query<WsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, room_id, query.user_id, state))
}

async fn handle_socket(socket: WebSocket, room_id: RoomId, user_id: UserId, state: AppState) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Register sender for this room
    state
        .room_txs
        .entry(room_id)
        .or_insert_with(Vec::new)
        .push(tx.clone());

    // Split socket into sender and receiver
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Spawn task to forward messages from the broadcast channel to the websocket
    let mut send_task = tokio::spawn(async move {
        while let Some(msg_result) = rx.recv().await {
            match msg_result {
                Ok(msg) => {
                    if ws_sender.send(msg).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
                    // Log the error and continue with other messages
                    warn!("Error receiving message from channel: {e}");
                }
            }
        }
    });

    // Spawn task to handle incoming WebSocket messages
    let state_clone = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(AxumMessage::Text(text))) = ws_receiver.next().await {
            if let Ok(incoming) = serde_json::from_str::<NewMessage>(&text) {
                if incoming.room_id() == &room_id && incoming.user_id() == &user_id {
                    // Store in DB
                    if let Err(e) =
                        insert_message(&state_clone.pool, user_id, room_id, incoming.content())
                            .await
                    {
                        error!("DB insert error: {e}");
                        continue;
                    }

                    // Get user info for broadcast
                    let user_row: Option<(String, UserRole)> =
                        sqlx::query_as("SELECT name, role FROM users WHERE id = ?")
                            .bind(user_id)
                            .fetch_optional(&state_clone.pool)
                            .await
                            .unwrap_or(None);

                    let (user_name, role) = match user_row {
                        Some((name, role)) => (name, role),
                        None => {
                            error!("User with id {user_id} not found in DB");
                            continue;
                        }
                    };

                    let broadcast_msg: Utf8Bytes = serde_json::json!(&MessagePayload::new(
                        user_name,
                        role,
                        incoming.content().to_string(),
                        chrono::Utc::now().timestamp_millis()
                    ))
                    .to_string()
                    .into();

                    if let Some(senders) = state_clone.room_txs.get(&room_id) {
                        for tx in senders.value() {
                            let _ = tx.send(Ok(AxumMessage::Text(broadcast_msg.clone())));
                        }
                    }
                }
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    // Remove the sender from the room map
    if let Some(mut senders) = state.room_txs.get_mut(&room_id) {
        senders.retain(|s| !s.is_closed());
    }
}

// ----- Server Startup with ACME for certs -----
/// Run HTTPS server with automatic Let's Encrypt certificates
async fn run_https_server(app: Router, domain: String, email: String) -> anyhow::Result<()> {
    let acme_state = AcmeConfig::new([domain.clone()])
        .contact_push(&format!("mailto:{}", email))
        .cache(DirCache::new("./acme_cache"))
        .directory_lets_encrypt(true)
        .state();

    let rustls_config = acme_state.default_rustls_config();
    let acceptor = acme_state.axum_acceptor(rustls_config);

    let mut renewal_state = acme_state;
    tokio::spawn(async move {
        while let Some(event) = renewal_state.next().await {
            match event {
                Ok(ok) => info!("ACME event: {:?}", ok),
                Err(err) => error!("ACME error: {:?}", err),
            }
        }
    });

    let addr: SocketAddr = "0.0.0.0:443".parse()?;
    info!("Starting HTTPS server on https://{}", domain);
    axum_server::bind(addr)
        .acceptor(acceptor)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

async fn redirect_handler(uri: Uri, State(domain): State<String>) -> Redirect {
    let target = format!(
        "https://{}{}",
        domain,
        uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/")
    );
    Redirect::permanent(&target)
}

async fn run_http_server(app: Option<Router>, domain: String, port: u16) -> anyhow::Result<()> {
    let app = match app {
        Some(app) => app,
        None => Router::new().fallback(redirect_handler).with_state(domain),
    };
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Starting HTTP server on http://{}", addr);
    axum_server::bind(addr)
        .serve(app.into_make_service())
        .await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let pool = sqlx::SqlitePool::connect(
        &std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:chat.db".to_string()),
    )
    .await?;
    init_db(&pool).await?;

    sqlx::query("DELETE FROM Messages").execute(&pool).await?;

    let templates = tera::Tera::new("templates/*.html")?;
    let state = AppState {
        pool,
        room_txs: Arc::new(DashMap::new()),
        templates,
    };

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/rooms", get(room_list_handler))
        .route("/room/{room_id}", get(room_handler))
        .route("/api/users", post(create_user_handler))
        .route("/api/rooms", get(get_rooms_handler))
        .route("/api/rooms/{room_id}/history", get(history_handler))
        .route("/ws/{room_id}", get(ws_handler))
        .with_state(state);

    if std::env::var("USE_ACME")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true"
    {
        let domain = std::env::var("DOMAIN").expect("DOMAIN and EMAIL required when USE_ACME=true");
        let email = std::env::var("EMAIL").expect("DOMAIN and EMAIL required when USE_ACME=true");
        // Run HTTPS server on 443 and HTTP redirector on 80
        tokio::try_join!(
            run_https_server(app, domain.clone(), email),
            run_http_server(None, domain, 80)
        )?;
    } else {
        let port = std::env::var("PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .unwrap_or(3000);
        run_http_server(Some(app), "localhost".to_string(), port).await?;
    }

    Ok(())
}
