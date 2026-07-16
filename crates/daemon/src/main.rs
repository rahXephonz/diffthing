//! diffthing daemon. Boot sequence:
//!   1. detect git repo
//!   2. pick free port, generate session token
//!   3. initial diff -> score -> walkthrough (LLM w/ validator gate, or fallback)
//!   4. start watcher (debounce-to-quiescence) + axum server (WS, token+origin gated)
//!   5. print the URL — token in the FRAGMENT, never the query string

mod gitio;
mod llm;
mod server;
mod session;
mod watcher;

use clap::Parser;
use rand::Rng;
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

pub const HOSTED_ORIGIN: &str = "https://local.diffthing.dev";
pub const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(name = "diffthing", about = "AI organizes the diff. Only you review.")]
struct Cli {
    /// Diff base. Default: working tree vs HEAD (uncommitted agent output).
    #[arg(long, default_value = "HEAD")]
    base: String,
    /// Serve the embedded SPA from 127.0.0.1 instead of the hosted origin.
    #[arg(long)]
    offline: bool,
    /// Fixed port (default: first free port).
    #[arg(long)]
    port: Option<u16>,
    /// Repo root (default: cwd).
    #[arg(long)]
    repo: Option<PathBuf>,
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind to ephemeral port")
        .local_addr()
        .expect("local addr")
        .port()
}

fn gen_token() -> String {
    let bytes: [u8; 24] = rand::thread_rng().gen();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[tokio::main]
async fn main() -> anyhow_lite::Result<()> {
    let cli = Cli::parse();
    let repo = cli.repo.unwrap_or_else(|| std::env::current_dir().expect("cwd"));

    if !gitio::is_git_repo(&repo) {
        eprintln!("diffthing: {} is not a git repository", repo.display());
        std::process::exit(1);
    }

    let port = cli.port.unwrap_or_else(free_port);
    let token = gen_token();

    let session = Arc::new(session::Session::boot(&repo, &cli.base, token.clone()).await?);

    // Watcher feeds the session's reconciliation loop.
    watcher::spawn(repo.clone(), Arc::clone(&session));

    let url = if cli.offline {
        format!("http://127.0.0.1:{port}/#port={port}&token={token}")
    } else {
        format!("{HOSTED_ORIGIN}/#port={port}&token={token}")
    };
    println!();
    println!("  diffthing {DAEMON_VERSION}");
    println!("  reviewing {} against {}", repo.display(), cli.base);
    println!();
    println!("  open  {url}");
    println!();

    server::serve(port, session).await
}

/// Tiny local Result alias to avoid pulling anyhow for the scaffold.
mod anyhow_lite {
    pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
}
