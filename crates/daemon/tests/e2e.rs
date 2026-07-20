//! End-to-end daemon tests: spawn the REAL compiled binary against a real
//! temp git repo and drive it over HTTP + WebSocket, exactly like the SPA
//! does. This is the only place `main`, `serve`, and `ServeMode` run under
//! test — unit tests can't reach them.
//!
//! Every daemon runs `--offline --llm none`: plain HTTP on loopback, and the
//! deterministic fallback walkthrough so no agent CLI is ever invoked.

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

const PROTOCOL_VERSION: u16 = 5;
const WAIT: Duration = Duration::from_secs(30);

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// Fresh git repo with one committed file plus an uncommitted change, so the
/// daemon boots with a non-empty diff against HEAD.
fn setup_repo() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("diffthing-e2e-{:08x}", rand_id()));
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["config", "user.email", "e2e@test"]);
    git(&dir, &["config", "user.name", "e2e"]);
    std::fs::write(dir.join("app.txt"), "line one\nline two\n").unwrap();
    git(&dir, &["add", "."]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    std::fs::write(dir.join("app.txt"), "line one\nline two\nline three\n").unwrap();
    dir
}

fn rand_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    // Clock alone is not unique enough: concurrent tests observe the same
    // timestamp. The counter disambiguates within a process, the timestamp
    // across runs (stale dirs from a killed earlier run).
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    (t.as_nanos() as u64)
        ^ ((std::process::id() as u64) << 32)
        ^ (SEQ.fetch_add(1, Ordering::Relaxed) << 48)
}

struct Daemon {
    child: Child,
    port: u16,
    token: String,
}

impl Daemon {
    /// A dead daemon turns every later timeout into noise — fail loudly at
    /// the moment of death instead.
    fn assert_alive(&mut self) {
        if let Ok(Some(status)) = self.child.try_wait() {
            panic!("daemon exited early with {status} (its stderr is above)");
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spawn the daemon and block until it prints its URL, from which the port
/// and session token are parsed — the same channel a human uses. The daemon
/// picks its own free port: tests choosing one up front and releasing it
/// races other concurrently spawning tests onto the same port.
fn spawn_daemon(repo: &Path) -> Daemon {
    let mut child = Command::new(env!("CARGO_BIN_EXE_diffthing"))
        .args(["--offline", "--llm", "none", "--repo"])
        .arg(repo)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit()) // daemon panics/errors belong in test output
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut parsed = None;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        // "  open  http://127.0.0.1:PORT/#port=PORT&token=TOKEN"
        if let Some(rest) = line.trim().strip_prefix("open") {
            let url = rest.trim();
            let port = url
                .split("#port=")
                .nth(1)
                .and_then(|s| s.split('&').next())
                .and_then(|s| s.parse::<u16>().ok());
            let token = url.split("token=").nth(1).map(str::to_string);
            parsed = port.zip(token);
            break;
        }
    }
    let (port, token) = parsed.expect("daemon printed no URL with a port and token");
    Daemon { child, port, token }
}

/// Minimal HTTP GET over raw TCP — enough to probe /health and the SPA
/// without pulling an HTTP client into the tree.
async fn http_get(port: u16, path: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let deadline = tokio::time::Instant::now() + WAIT;
    loop {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)).await {
            let req =
                format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
            if stream.write_all(req.as_bytes()).await.is_ok() {
                let mut buf = String::new();
                if stream.read_to_string(&mut buf).await.is_ok() && !buf.is_empty() {
                    return buf;
                }
            }
        }
        assert!(tokio::time::Instant::now() < deadline, "daemon never answered {path}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn ws_connect(daemon: &mut Daemon) -> Ws {
    let deadline = tokio::time::Instant::now() + WAIT;
    let port = daemon.port;
    loop {
        daemon.assert_alive();
        match tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws")).await {
            Ok((ws, _)) => return ws,
            Err(e) => {
                assert!(
                    tokio::time::Instant::now() < deadline,
                    "ws never connected; last error: {e:?}"
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

async fn send_json(ws: &mut Ws, v: Value) {
    ws.send(Message::Text(v.to_string())).await.unwrap();
}

/// Read messages until one with the wanted `type` arrives. Unrelated
/// broadcasts (progress, review echoes) are skipped, not errors.
async fn recv_type(ws: &mut Ws, want: &str) -> Value {
    let fut = async {
        while let Some(msg) = ws.next().await {
            if let Message::Text(t) = msg.expect("ws closed mid-test") {
                let v: Value = serde_json::from_str(&t).unwrap();
                if v["type"] == want {
                    return v;
                }
            }
        }
        panic!("stream ended before a `{want}` message");
    };
    tokio::time::timeout(WAIT, fut)
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for `{want}`"))
}

/// Handshake as the SPA does; returns the post-hello snapshot.
async fn handshake(ws: &mut Ws, token: &str) -> Value {
    send_json(ws, json!({ "type": "hello", "protocol": PROTOCOL_VERSION, "token": token })).await;
    let ack = recv_type(ws, "hello_ack").await;
    assert_eq!(ack["protocol"], PROTOCOL_VERSION);
    recv_type(ws, "snapshot").await
}

fn first_hunk_id(snapshot: &Value) -> String {
    snapshot["files"][0]["hunks"][0]["id"].as_str().expect("snapshot has a hunk").to_string()
}

#[tokio::test(flavor = "multi_thread")]
async fn boot_serves_health_and_spa() {
    let repo = setup_repo();
    let daemon = spawn_daemon(&repo);

    let health = http_get(daemon.port, "/health").await;
    assert!(health.starts_with("HTTP/1.1 200"), "health: {health}");
    assert!(health.contains("\"ok\":true"));
    assert!(health.contains(&format!("\"protocol\":{PROTOCOL_VERSION}")));

    // ServeMode::OfflineHttp serves the embedded SPA off the same port.
    let index = http_get(daemon.port, "/").await;
    assert!(index.starts_with("HTTP/1.1 200"), "index: {index}");
    assert!(index.contains("text/html"));

    // Missing assets are honest 404s, not index.html fallbacks.
    let missing = http_get(daemon.port, "/assets/nope.js").await;
    assert!(missing.starts_with("HTTP/1.1 404"), "asset: {missing}");

    drop(daemon);
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn ws_handshake_review_roundtrip_and_restart_resume() {
    let repo = setup_repo();
    let mut daemon = spawn_daemon(&repo);

    let mut ws = ws_connect(&mut daemon).await;
    let snapshot = handshake(&mut ws, &daemon.token).await;

    // --llm none ⇒ deterministic fallback, honestly labeled.
    assert_eq!(snapshot["walkthrough"]["degraded"], true);
    assert_eq!(snapshot["files"][0]["path"], "app.txt");
    let hunk = first_hunk_id(&snapshot);

    // Open a flag FIRST: a fully viewed file with no open flags is staged as
    // approved and leaves the active review — the open thread pins it.
    send_json(&mut ws, json!({ "type": "add_flag", "hunk": hunk, "comment": "why three?" })).await;
    let updated = recv_type(&mut ws, "review_updated").await;
    assert_eq!(updated["review"]["flags"][0]["thread"][0]["body"], "why three?");

    // Mark viewed → daemon echoes ReviewUpdated with the new status.
    send_json(&mut ws, json!({ "type": "mark_viewed", "hunk": hunk })).await;
    let updated = recv_type(&mut ws, "review_updated").await;
    assert_eq!(updated["review"]["status"][&hunk], "viewed");

    // Restart the daemon (new process, new token). Persisted review must
    // resume: same content hash, same viewed mark, same open flag.
    drop(ws);
    drop(daemon);
    let mut daemon2 = spawn_daemon(&repo);
    assert_ne!(daemon2.token, "", "fresh token expected");
    let mut ws2 = ws_connect(&mut daemon2).await;
    let snapshot2 = handshake(&mut ws2, &daemon2.token).await;
    assert_eq!(snapshot2["review"]["status"][&hunk], "viewed", "viewed mark must survive restart");
    assert_eq!(snapshot2["review"]["flags"][0]["thread"][0]["body"], "why three?");

    drop(ws2);
    drop(daemon2);
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn ws_rejects_bad_token_and_protocol_mismatch() {
    let repo = setup_repo();
    let mut daemon = spawn_daemon(&repo);

    // Wrong token: explicit BadToken error, then nothing.
    let mut ws = ws_connect(&mut daemon).await;
    send_json(&mut ws, json!({ "type": "hello", "protocol": PROTOCOL_VERSION, "token": "wrong" }))
        .await;
    let err = recv_type(&mut ws, "error").await;
    assert_eq!(err["code"], "bad_token");

    // Right token, wrong protocol: ProtocolMismatch.
    let mut ws = ws_connect(&mut daemon).await;
    send_json(&mut ws, json!({ "type": "hello", "protocol": 1, "token": daemon.token })).await;
    let err = recv_type(&mut ws, "error").await;
    assert_eq!(err["code"], "protocol_mismatch");

    drop(daemon);
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn ws_rejects_disallowed_origin() {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let repo = setup_repo();
    let daemon = spawn_daemon(&repo);

    // Wait until the server is actually up before the one-shot rejection.
    http_get(daemon.port, "/health").await;

    let mut req = format!("ws://127.0.0.1:{}/ws", daemon.port).into_client_request().unwrap();
    req.headers_mut().insert("origin", "https://evil.example".parse().unwrap());
    let err = tokio_tungstenite::connect_async(req).await.expect_err("upgrade must be refused");
    match err {
        tokio_tungstenite::tungstenite::Error::Http(resp) => {
            assert_eq!(resp.status(), 403);
        }
        other => panic!("expected HTTP 403 rejection, got: {other:?}"),
    }

    drop(daemon);
    std::fs::remove_dir_all(&repo).ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn watcher_reconciliation_announces_and_applies() {
    let repo = setup_repo();
    let mut daemon = spawn_daemon(&repo);

    let mut ws = ws_connect(&mut daemon).await;
    let snapshot = handshake(&mut ws, &daemon.token).await;
    let revision_before = snapshot["walkthrough"]["revision"].as_u64().unwrap();

    // Move the tree under the daemon. Watcher debounces to quiescence, then
    // announces — the served snapshot must NOT advance on its own.
    std::fs::write(repo.join("app.txt"), "line one\nline two\nline three\nline four\n").unwrap();

    let update = recv_type(&mut ws, "update_available").await;
    let revision = update["revision"].as_u64().unwrap();
    assert!(revision > revision_before);

    // Client applies; NOW the snapshot advances and reflects the new tree.
    send_json(&mut ws, json!({ "type": "apply_update", "to_revision": revision })).await;
    let snapshot = recv_type(&mut ws, "snapshot").await;
    assert_eq!(snapshot["walkthrough"]["revision"].as_u64().unwrap(), revision);
    let lines = snapshot["files"][0]["hunks"][0]["lines"].to_string();
    assert!(lines.contains("line four"), "applied snapshot must contain the new line: {lines}");

    drop(daemon);
    std::fs::remove_dir_all(&repo).ok();
}
