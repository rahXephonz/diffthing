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
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use diffthing_core::protocol::{ClientMsg, ErrorCode, ServerMsg, PROTOCOL_VERSION};
use rust_embed::RustEmbed;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

// Resource limits (DoS hardening). The daemon is loopback-only, but a
// malicious local process or a token-holding tab must not be able to exhaust
// tasks, memory, or spawn unbounded LLM work.
/// Concurrent WS connections — far above any real number of local tabs.
const MAX_WS_CONNECTIONS: usize = 16;
/// Max WS message/frame size. Client messages are small JSON (comments,
/// instructions); 256 KiB is generous, not open-ended.
const MAX_WS_MSG_BYTES: usize = 256 * 1024;
/// A socket that hasn't sent its Hello within this window is dropped —
/// pre-auth sockets must not park forever.
const HELLO_DEADLINE: Duration = Duration::from_secs(10);
/// Per-connection token bucket for client messages: sustained rate and burst.
/// Humans clicking through a review sit far below both.
const MSG_BURST: f64 = 30.0;
const MSG_REFILL_PER_SEC: f64 = 10.0;

/// Token bucket: refills continuously, capped at burst. One per connection.
struct RateLimiter {
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    fn new() -> Self {
        Self { tokens: MSG_BURST, last: Instant::now() }
    }

    fn allow(&mut self) -> bool {
        let now = Instant::now();
        self.tokens = MSG_BURST
            .min(self.tokens + now.duration_since(self.last).as_secs_f64() * MSG_REFILL_PER_SEC);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

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
    /// Hard cap on live WS connections; permits are held for the socket's
    /// whole lifetime, so leaked/parked sockets count until they close.
    conns: Arc<Semaphore>,
}

/// Serves on an ALREADY-BOUND listener (bound at boot, before the URL was
/// printed) — rebinding here would race other processes onto a port the
/// user was already told to open.
pub async fn serve(
    listener: std::net::TcpListener,
    session: Arc<Session>,
    mode: ServeMode,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let port = listener.local_addr()?.port();
    // The daemon serves the SPA itself in both modes, so the page and its WS
    // target always share one origin — the browser never sees a cross-origin
    // or mixed-content request.
    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_upgrade))
        .with_state(AppState { session, port, conns: Arc::new(Semaphore::new(MAX_WS_CONNECTIONS)) })
        .fallback(get(serve_asset))
        .layer(axum::middleware::from_fn(security_headers));

    listener.set_nonblocking(true)?;
    match mode {
        ServeMode::HostedTls { cert_pem, key_pem } => {
            let config = RustlsConfig::from_pem(cert_pem, key_pem).await?;
            axum_server::from_tcp_rustls(listener, config).serve(app.into_make_service()).await?;
        }
        ServeMode::OfflineHttp => {
            let listener = tokio::net::TcpListener::from_std(listener)?;
            axum::serve(listener, app).await?;
        }
    }
    Ok(())
}

/// Baseline browser protections on every response (defense in depth — the
/// daemon serves only its own embedded SPA, so the policy can be strict).
/// `connect-src` lists the WS origins explicitly: CSP3 lets `'self'` match
/// same-origin ws/wss, but Safari historically does not, and the SPA always
/// dials `location.host` — hosted TLS or loopback.
const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
     img-src 'self' data:; font-src 'self'; \
     connect-src 'self' wss://local.diffthing.dev:* ws://127.0.0.1:* ws://localhost:*; \
     object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

async fn security_headers(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let h = resp.headers_mut();
    h.insert("content-security-policy", HeaderValue::from_static(CSP));
    h.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    resp
}

/// Origin policy, shared by `/health` and `/ws`:
///   - ABSENT header  = non-browser client (curl, scripts) — allowed, the
///     session token still gates everything that matters.
///   - PRESENT header = must be UTF-8 AND allowlisted. A malformed value
///     must never fall through to the more permissive "absent" branch.
enum OriginCheck {
    Absent,
    Allowed(String),
    Denied,
}

fn check_origin(headers: &HeaderMap, port: u16) -> OriginCheck {
    match headers.get("origin") {
        None => OriginCheck::Absent,
        Some(v) => match v.to_str() {
            Ok(o) if origin_allowed(o, port) => OriginCheck::Allowed(o.to_string()),
            _ => OriginCheck::Denied,
        },
    }
}

/// Probe endpoint for the SPA's connection diagnosis: if this answers but
/// the WS fails, the problem is the browser (shields/PNA), not the daemon.
/// CORS-gated the same as `/ws`: the hosted SPA and an offline-mode tab are
/// both cross-origin from the daemon's own port, so a plain fetch needs the
/// allowlisted origin echoed back or the browser drops the response.
async fn health(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let origin = match check_origin(&headers, state.port) {
        OriginCheck::Denied => return StatusCode::FORBIDDEN.into_response(),
        OriginCheck::Absent => None,
        OriginCheck::Allowed(o) => Some(o),
    };
    let mut resp = axum::Json(serde_json::json!({
        "ok": true,
        "daemon": DAEMON_VERSION,
        "protocol": PROTOCOL_VERSION,
    }))
    .into_response();
    if let Some(origin) = origin {
        if let Ok(v) = HeaderValue::from_str(&origin) {
            resp.headers_mut().insert(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, v);
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
    if matches!(check_origin(&headers, state.port), OriginCheck::Denied) {
        return StatusCode::FORBIDDEN.into_response();
    }
    // Connection cap: refuse outright rather than queueing — a flood must
    // not build a backlog of pending upgrades.
    let Ok(permit) = Arc::clone(&state.conns).try_acquire_owned() else {
        return axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    ws.max_message_size(MAX_WS_MSG_BYTES)
        .max_frame_size(MAX_WS_MSG_BYTES)
        .on_upgrade(move |socket| handle_ws(socket, state.session, permit))
        .into_response()
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

async fn handle_ws(mut socket: WebSocket, session: Arc<Session>, _permit: OwnedSemaphorePermit) {
    // Message #1 MUST be Hello{protocol, token}, and it must arrive within
    // the deadline — an idle pre-auth socket is dropped, not parked.
    let hello = match tokio::time::timeout(HELLO_DEADLINE, socket.recv()).await {
        Ok(Some(Ok(Message::Text(t)))) => serde_json::from_str::<ClientMsg>(&t).ok(),
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
    let mut limiter = RateLimiter::new();

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
                // Mutation flood control: excess messages are refused, not
                // queued. The client is told why and the socket stays open.
                if !limiter.allow() {
                    let ok = send(&mut socket, &ServerMsg::Error {
                        code: ErrorCode::RateLimited,
                        message: "too many requests — slow down".into(),
                    }).await;
                    if !ok { return; }
                    continue;
                }
                match msg {
                    ClientMsg::MarkViewed { hunk } => {
                        session.mark_viewed(hunk).await;
                        broadcast_review(&session).await;
                    }
                    ClientMsg::MarkAllViewed => {
                        session.mark_all_viewed().await;
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
                        session.persist().await;
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
                        session.persist().await;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limiter_allows_burst_then_refuses() {
        let mut rl = RateLimiter::new();
        for i in 0..MSG_BURST as usize {
            assert!(rl.allow(), "message {i} within burst must pass");
        }
        assert!(!rl.allow(), "burst exhausted — must refuse");
    }

    #[test]
    fn rate_limiter_refills_over_time() {
        let mut rl = RateLimiter::new();
        for _ in 0..MSG_BURST as usize {
            rl.allow();
        }
        assert!(!rl.allow());
        // Simulate one second passing: refill grants MSG_REFILL_PER_SEC.
        rl.last = Instant::now() - Duration::from_secs(1);
        for i in 0..MSG_REFILL_PER_SEC as usize {
            assert!(rl.allow(), "refilled message {i} must pass");
        }
        assert!(!rl.allow(), "refill spent — must refuse again");
    }
}
