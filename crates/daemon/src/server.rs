//! HTTP + WS server, loopback only. Security model (non-negotiable):
//!   - bind 127.0.0.1 exclusively
//!   - token required on the WS handshake (from the URL fragment, sent by
//!     the SPA as the first message — fragments never hit servers or logs)
//!   - Origin allowlist: hosted origin + own loopback origin (offline mode)
//!   - protocol version handshake is message #1 in both directions
//!
//! This is the defense against a malicious tab dialing the daemon.

use crate::session::Session;
use crate::{hosted_origin, DAEMON_VERSION};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use diffthing_core::protocol::{ClientMsg, ErrorCode, ServerMsg, PROTOCOL_VERSION};
use rust_embed::RustEmbed;
use std::net::SocketAddr;
use std::sync::Arc;

/// How the daemon exposes the review UI.
pub enum ServeMode {
    /// Default: HTTPS on `127.0.0.1`, reached via `local.diffthing.dev` which
    /// resolves to loopback. SPA + WS are same-origin over TLS. Carries the
    /// PEM cert chain + key to serve.
    HostedTls { cert_pem: Vec<u8>, key_pem: Vec<u8> },
    /// `--offline`: plain HTTP on `127.0.0.1`. No DNS or cert dependency.
    OfflineHttp,
}

/// Built by `pnpm --filter diffthing-web build` — must exist at compile
/// time. `--offline` serves this off the daemon's own port so the page and
/// the WS target share one origin, sidestepping browser Local Network
/// Access / Private Network Access permission gates entirely.
#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../web/dist/"]
struct WebAssets;

#[derive(Clone)]
struct AppState {
    session: Arc<Session>,
    port: u16,
}

pub async fn serve(
    port: u16,
    session: Arc<Session>,
    mode: ServeMode,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // The daemon serves the SPA itself in both modes, so the page and its WS
    // target always share one origin — the browser never sees a cross-origin
    // or mixed-content request.
    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_upgrade))
        .with_state(AppState { session, port })
        .fallback(get(serve_asset));

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    match mode {
        ServeMode::HostedTls { cert_pem, key_pem } => {
            let config = RustlsConfig::from_pem(cert_pem, key_pem).await?;
            axum_server::bind_rustls(addr, config).serve(app.into_make_service()).await?;
        }
        ServeMode::OfflineHttp => {
            let listener = tokio::net::TcpListener::bind(addr).await?;
            axum::serve(listener, app).await?;
        }
    }
    Ok(())
}

/// Probe endpoint for the SPA's connection diagnosis: if this answers but
/// the WS fails, the problem is the browser (shields/PNA), not the daemon.
/// CORS-gated the same as `/ws`: the hosted SPA and an offline-mode tab are
/// both cross-origin from the daemon's own port, so a plain fetch needs the
/// allowlisted origin echoed back or the browser drops the response.
async fn health(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let mut resp = axum::Json(serde_json::json!({
        "ok": true,
        "daemon": DAEMON_VERSION,
        "protocol": PROTOCOL_VERSION,
    }))
    .into_response();
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        if origin_allowed(origin, state.port) {
            if let Ok(v) = HeaderValue::from_str(origin) {
                resp.headers_mut().insert(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, v);
            }
        }
    }
    resp
}

fn origin_allowed(origin: &str, port: u16) -> bool {
    // Hosted TLS: the SPA is served from https://local.diffthing.dev:PORT
    // (same daemon, same port). Offline: plain-http loopback.
    if origin == hosted_origin(port)
        || origin == format!("http://127.0.0.1:{port}")
        || origin == format!("http://localhost:{port}")
    {
        return true;
    }
    // Dev-loop escape hatch for `pnpm dev` (vite on its own port) — compiled
    // out of release builds, so it can't widen the allowlist in anything
    // that ships. Opt-in only: unset means no extra origin, same as today.
    #[cfg(debug_assertions)]
    if let Ok(dev_origin) = std::env::var("DIFFTHING_DEV_ORIGIN") {
        return origin == dev_origin;
    }
    false
}

/// `--offline` fallback: serves the embedded SPA build off the daemon's own
/// port. Unknown paths fall back to `index.html` — this is a single-page
/// app, the only real routing is the URL hash, never the path.
async fn serve_asset(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    // SPA fallback belongs only to navigation routes. Returning index.html
    // for a missing JS/CSS asset makes browsers parse HTML as JavaScript and
    // hides the real problem behind `Unexpected token '<'`.
    let file = WebAssets::get(path).or_else(|| {
        (!path.contains('.') && !path.starts_with("assets/"))
            .then(|| WebAssets::get("index.html"))
            .flatten()
    });
    match file {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(axum::http::header::CONTENT_TYPE, mime.as_ref().to_string())], file.data)
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn ws_upgrade(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Non-browser clients (curl) send no Origin; allow — token still gates.
    let allowed = match headers.get("origin").and_then(|v| v.to_str().ok()) {
        Some(o) => origin_allowed(o, state.port),
        None => true,
    };
    if !allowed {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    ws.on_upgrade(move |socket| handle_ws(socket, state.session)).into_response()
}

/// Push the current review state to every connected tab. Called after any
/// mutation that changes review state without moving the diff, so a comment
/// or viewed-mark shows instantly instead of waiting for a reconnect.
async fn broadcast_review(session: &Session) {
    let review = session.state.lock().await.review.clone();
    let _ = session.events.send(ServerMsg::ReviewUpdated { review });
}

async fn send(socket: &mut WebSocket, msg: &ServerMsg) -> bool {
    match serde_json::to_string(msg) {
        Ok(s) => socket.send(Message::Text(s)).await.is_ok(),
        Err(_) => false,
    }
}

async fn handle_ws(mut socket: WebSocket, session: Arc<Session>) {
    // Message #1 MUST be Hello{protocol, token}.
    let hello = match socket.recv().await {
        Some(Ok(Message::Text(t))) => serde_json::from_str::<ClientMsg>(&t).ok(),
        _ => None,
    };
    match hello {
        Some(ClientMsg::Hello { protocol, token }) => {
            if token != session.token {
                let _ = send(
                    &mut socket,
                    &ServerMsg::Error {
                        code: ErrorCode::BadToken,
                        message: "session ended — rerun diffthing for a new URL".into(),
                    },
                )
                .await;
                return;
            }
            if protocol != PROTOCOL_VERSION {
                let _ = send(
                    &mut socket,
                    &ServerMsg::Error {
                        code: ErrorCode::ProtocolMismatch,
                        message: format!(
                            "daemon speaks protocol v{PROTOCOL_VERSION}, client v{protocol} — run `npx diffthing@latest`"
                        ),
                    },
                )
                .await;
                return;
            }
        }
        _ => return,
    }

    if !send(
        &mut socket,
        &ServerMsg::HelloAck {
            protocol: PROTOCOL_VERSION,
            daemon_version: DAEMON_VERSION.into(),
            llm: session.llm_label(),
        },
    )
    .await
    {
        return;
    }

    // Full snapshot on connect — the browser is stateless.
    {
        let state = session.state.lock().await;
        let snap = ServerMsg::Snapshot {
            walkthrough: state.walkthrough.clone(),
            files: state.files.clone(),
            scores: state.scores.clone(),
            review: state.review.clone(),
        };
        if !send(&mut socket, &snap).await {
            return;
        }
    }

    let mut events = session.events.subscribe();

    loop {
        tokio::select! {
            ev = events.recv() => {
                // Err(_): lagged — snapshot-on-apply covers recovery
                if let Ok(msg) = ev {
                    if !send(&mut socket, &msg).await { return; }
                }
            }
            incoming = socket.recv() => {
                let Some(Ok(Message::Text(t))) = incoming else { return };
                let Ok(msg) = serde_json::from_str::<ClientMsg>(&t) else { continue };
                match msg {
                    ClientMsg::MarkViewed { hunk } => {
                        session.mark_viewed(hunk).await;
                        broadcast_review(&session).await;
                    }
                    ClientMsg::AddFlag { hunk, line, comment } => {
                        // Thread semantics: a comment on a (hunk, line) that
                        // already has an open thread is a REPLY (appended);
                        // otherwise it opens a new one. One open thread per
                        // anchor point, GitHub-style.
                        use diffthing_core::review::{Flag, FlagEntryKind};
                        {
                            let mut st = session.state.lock().await;
                            let rev = st.walkthrough.revision;
                            match st
                                .review
                                .flags
                                .iter_mut()
                                .find(|f| f.hunk == hunk && f.line == line && f.open)
                            {
                                Some(f) => f.push(FlagEntryKind::HumanComment, comment, rev),
                                None => st.review.flags.push(Flag::new(hunk, line, comment)),
                            }
                        }
                        broadcast_review(&session).await;
                    }
                    ClientMsg::CloseFlag { hunk, line } => {
                        // Closing a flag is a human click — the only path.
                        {
                            let mut st = session.state.lock().await;
                            for f in st.review.flags.iter_mut() {
                                if f.hunk == hunk && f.line == line { f.open = false; }
                            }
                        }
                        // Last open flag may have been the only reason a
                        // fully viewed file could not enter staged changes.
                        session.stage_if_approved(&hunk).await;
                        broadcast_review(&session).await;
                    }
                    ClientMsg::ApplyUpdate { to_revision } => {
                        if session.apply_update(to_revision).await {
                            let state = session.state.lock().await;
                            let snap = ServerMsg::Snapshot {
                                walkthrough: state.walkthrough.clone(),
                                files: state.files.clone(),
                                scores: state.scores.clone(),
                                review: state.review.clone(),
                            };
                            drop(state);
                            if !send(&mut socket, &snap).await { return; }
                            // Preserve established scope order, but replace
                            // orphan placeholder framing with validated AI
                            // organization in the background.
                            session.spawn_new_changes_upgrade();
                        }
                    }
                    ClientMsg::ExportReview => {
                        let md = session.export_markdown().await;
                        if !send(&mut socket, &ServerMsg::ReviewExport { markdown: md }).await {
                            return;
                        }
                    }
                    ClientMsg::RequestChange { hunks, line, instruction, runner } => {
                        // Anchored dispatch (inv 9): always attached to hunks.
                        // The task snapshots, single-writer-locks, runs the
                        // agent, scope-checks, and announces status over the
                        // broadcast channel. Results re-enter via the watcher.
                        crate::dispatch::spawn(
                            Arc::clone(&session), hunks, line, instruction, runner,
                        );
                    }
                    ClientMsg::Regenerate => {
                        session.spawn_walkthrough_upgrade();
                    }
                    ClientMsg::Hello { .. } => {} // already handshaken
                }
            }
        }
    }
}
