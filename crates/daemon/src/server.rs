//! HTTP + WS server, loopback only. Security model (non-negotiable):
//!   - bind 127.0.0.1 exclusively
//!   - token required on the WS handshake (from the URL fragment, sent by
//!     the SPA as the first message — fragments never hit servers or logs)
//!   - Origin allowlist: hosted origin + own loopback origin (offline mode)
//!   - protocol version handshake is message #1 in both directions
//! This is the defense against a malicious tab dialing the daemon.

use crate::session::Session;
use crate::{DAEMON_VERSION, HOSTED_ORIGIN};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use diffthing_core::protocol::{ClientMsg, ErrorCode, ServerMsg, PROTOCOL_VERSION};
use std::net::SocketAddr;
use std::sync::Arc;

pub async fn serve(
    port: u16,
    session: Arc<Session>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_upgrade))
        .with_state(session);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Probe endpoint for the SPA's connection diagnosis: if this answers but
/// the WS fails, the problem is the browser (shields/PNA), not the daemon.
async fn health() -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "ok": true,
        "daemon": DAEMON_VERSION,
        "protocol": PROTOCOL_VERSION,
    }))
}

fn origin_allowed(headers: &HeaderMap, port: u16) -> bool {
    match headers.get("origin").and_then(|v| v.to_str().ok()) {
        Some(o) => {
            o == HOSTED_ORIGIN
                || o == format!("http://127.0.0.1:{port}")
                || o == format!("http://localhost:{port}")
        }
        // Non-browser clients (curl) send no Origin; allow — token still gates.
        None => true,
    }
}

async fn ws_upgrade(
    State(session): State<Arc<Session>>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let port = 0; // Origin port check refined in M1 (pass real port via state).
    if !origin_allowed(&headers, port) {
        return axum::http::StatusCode::FORBIDDEN.into_response();
    }
    ws.on_upgrade(move |socket| handle_ws(socket, session))
        .into_response()
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
                match ev {
                    Ok(msg) => { if !send(&mut socket, &msg).await { return; } }
                    Err(_) => {} // lagged — snapshot-on-apply covers recovery
                }
            }
            incoming = socket.recv() => {
                let Some(Ok(Message::Text(t))) = incoming else { return };
                let Ok(msg) = serde_json::from_str::<ClientMsg>(&t) else { continue };
                match msg {
                    ClientMsg::MarkViewed { hunk } => {
                        session.state.lock().await.review.mark_viewed(hunk);
                    }
                    ClientMsg::AddFlag { hunk, comment } => {
                        session.state.lock().await.review.flags.push(
                            diffthing_core::review::Flag {
                                hunk, comment, open: true, addressed_claim: false,
                            },
                        );
                    }
                    ClientMsg::CloseFlag { hunk } => {
                        // Closing a flag is a human click — the only path.
                        let mut st = session.state.lock().await;
                        for f in st.review.flags.iter_mut() {
                            if f.hunk == hunk { f.open = false; }
                        }
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
                        }
                    }
                    ClientMsg::ExportReview => {
                        let md = session.export_markdown().await;
                        if !send(&mut socket, &ServerMsg::ReviewExport { markdown: md }).await {
                            return;
                        }
                    }
                    ClientMsg::RequestChange { .. } => {
                        // M2: snapshot -> single-writer lock -> spawn runner
                        // (headless agent) -> scope validation. See CLAUDE.md.
                        let _ = send(&mut socket, &ServerMsg::Error {
                            code: ErrorCode::Internal,
                            message: "agent dispatch lands in M2".into(),
                        }).await;
                    }
                    ClientMsg::Regenerate => {
                        // M1: full regeneration via gated LLM pipeline.
                    }
                    ClientMsg::Hello { .. } => {} // already handshaken
                }
            }
        }
    }
}
