//! diffthing daemon. Boot sequence:
//!   1. detect git repo
//!   2. pick free port, generate session token
//!   3. initial diff -> score -> walkthrough (LLM w/ validator gate, or fallback)
//!   4. start watcher (debounce-to-quiescence) + axum server (WS, token+origin gated)
//!   5. print the URL — token in the FRAGMENT, never the query string

mod config;
mod dispatch;
mod gitio;
mod llm;
mod server;
mod session;
mod store;
mod tls;
mod watcher;

use clap::Parser;
use rand::Rng;
use server::ServeMode;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::{io::IsTerminal, io::Write};
use tokio::sync::oneshot;

/// Public DNS points this domain at `127.0.0.1`, so an HTTPS page served here
/// by the local daemon has a browser-trusted origin that reaches loopback.
pub const HOSTED_DOMAIN: &str = "local.diffthing.dev";
pub const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Origin the SPA is served from in hosted-TLS mode. Includes the port because
/// the daemon binds an ephemeral one and browser origins are host:port scoped.
pub fn hosted_origin(port: u16) -> String {
    format!("https://{HOSTED_DOMAIN}:{port}")
}

#[derive(Parser, Debug)]
#[command(name = "diffthing", about = "AI organizes the diff. Only you review.")]
struct Cli {
    /// Diff base. Default: working tree vs HEAD (uncommitted agent output).
    #[arg(long, default_value = "HEAD")]
    base: String,
    /// Serve over plain HTTP on 127.0.0.1 instead of HTTPS via
    /// local.diffthing.dev. Use when DNS or the cert can't be reached.
    #[arg(long)]
    offline: bool,
    /// Fixed port (default: first free port).
    #[arg(long)]
    port: Option<u16>,
    /// Repo root (default: cwd).
    #[arg(long)]
    repo: Option<PathBuf>,
    /// Agent CLI for walkthrough generation: claude | codex | gemini | kimi |
    /// qwen | opencode | none.
    /// Default auto: config.toml [llm] agent, else first installed on PATH.
    #[arg(long, default_value = "auto")]
    llm: String,
}

/// Bind the serving socket up front and KEEP it. Picking a free port and
/// releasing it invites another process onto the same port before `serve`
/// rebinds (concurrent daemons race exactly this way in CI).
fn bind_port(port: Option<u16>) -> std::io::Result<TcpListener> {
    TcpListener::bind(("127.0.0.1", port.unwrap_or(0)))
}

fn gen_token() -> String {
    let bytes: [u8; 24] = rand::thread_rng().gen();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

struct TerminalSpinner {
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl TerminalSpinner {
    fn start(message: String) -> Self {
        let (stop, mut stopped) = oneshot::channel();
        let interactive = std::io::stderr().is_terminal();
        if !interactive {
            eprintln!("  {message}...");
        }
        let task = tokio::spawn(async move {
            if !interactive {
                let _ = stopped.await;
                return;
            }
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let mut frame = 0;
            loop {
                eprint!("\r  \x1b[36m{}\x1b[0m {message}", frames[frame]);
                let _ = std::io::stderr().flush();
                tokio::select! {
                    _ = &mut stopped => break,
                    _ = tokio::time::sleep(std::time::Duration::from_millis(80)) => {
                        frame = (frame + 1) % frames.len();
                    }
                }
            }
            eprint!("\r\x1b[2K");
            let _ = std::io::stderr().flush();
        });
        Self { stop: Some(stop), task }
    }

    async fn finish(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let _ = self.task.await;
    }
}

#[tokio::main]
async fn main() -> anyhow_lite::Result<()> {
    let cli = Cli::parse();

    // rustls is pinned to the ring provider (no default installed). Do this
    // before any TLS use; harmless in offline mode.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let repo = cli.repo.unwrap_or_else(|| std::env::current_dir().expect("cwd"));

    if !gitio::is_git_repo(&repo) {
        eprintln!("diffthing: {} is not a git repository", repo.display());
        std::process::exit(1);
    }

    // Fail fast on option-shaped --base values (git argument injection).
    if let Err(e) = gitio::validate_base(&cli.base) {
        eprintln!("diffthing: {e}");
        std::process::exit(1);
    }

    let listener = bind_port(cli.port)?;
    let port = listener.local_addr()?.port();
    let token = gen_token();

    let llm_client = llm::AnyLlm::from_choice(config::resolve_agent(&cli.llm));
    let llm_desc = llm_client.describe();

    let spinner = TerminalSpinner::start(format!("scanning changes and organizing via {llm_desc}"));
    let session =
        Arc::new(session::Session::boot(&repo, &cli.base, token.clone(), llm_client).await?);
    spinner.finish().await;
    let (file_count, hunk_count, scope_count, degraded) = session.startup_counts().await;

    // Watcher feeds the session's reconciliation loop.
    watcher::spawn(repo.clone(), Arc::clone(&session));

    let hosted = !cli.offline;
    let url = if hosted {
        format!("{}/#port={port}&token={token}", hosted_origin(port))
    } else {
        format!("http://127.0.0.1:{port}/#port={port}&token={token}")
    };
    println!();
    println!("  diffthing {DAEMON_VERSION}");
    println!("  reviewing {} against {}", repo.display(), cli.base);
    println!("  llm       {llm_desc}");
    let ready =
        if std::io::stdout().is_terminal() { "\x1b[32m✓ ready\x1b[0m" } else { "✓ ready" };
    if degraded {
        println!(
            "  {ready}   {file_count} files, {hunk_count} changes, {scope_count} scopes (deterministic fallback)"
        );
    } else {
        println!(
            "  {ready}   {file_count} files, {hunk_count} changes, {scope_count} AI-organized scopes"
        );
    }
    let mode = if hosted {
        let (cert_pem, key_pem) = tls::material()?;
        ServeMode::HostedTls { cert_pem, key_pem }
    } else {
        ServeMode::OfflineHttp
    };

    println!();
    println!("  open  {url}");
    // Self-signed fallback: Safari refuses it entirely. Point those users at
    // the loopback-HTTP path instead of a dead page.
    if hosted && !tls::is_trusted() {
        println!(
            "  note  self-signed cert — Safari can't open this; use `npx diffthing --offline`"
        );
    }
    println!();

    server::serve(listener, session, mode).await
}

/// Tiny local Result alias to avoid pulling anyhow for the scaffold.
mod anyhow_lite {
    pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
}
