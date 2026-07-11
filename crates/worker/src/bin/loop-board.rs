//! Read-only factory/loop dashboard for `agents/monitor.sh`.
//!
//! Usage:
//!   loop-board [--repo <path>]
//!
//! Env:
//!   `DATABASE_URL`   — optional; registry section degrades when unset/down
//!   `GOVFOLIO_EPOCH` — default E2 (claimable/left scoped to this epoch)
//!
//! Exit: 0 always on successful render (degraded sections are not failures);
//! 2 on usage error.

use std::env;
use std::path::PathBuf;
use std::process::exit;

use worker::board;

fn usage_exit(msg: &str) -> ! {
    eprintln!("loop-board: {msg}\nusage: loop-board [--repo <path>]");
    exit(2);
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut repo: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--repo" => {
                let Some(p) = args.get(i + 1) else {
                    usage_exit("--repo needs a path");
                };
                repo = Some(PathBuf::from(p));
                i += 2;
            }
            "-h" | "--help" => {
                println!("usage: loop-board [--repo <path>]");
                return;
            }
            other => usage_exit(&format!("unknown arg {other:?}")),
        }
    }

    let repo = match repo {
        Some(p) => p,
        None => env::current_dir().unwrap_or_else(|e| {
            eprintln!("loop-board: cwd: {e}");
            exit(2);
        }),
    };

    let snap = board::collect(&repo).await;
    print!("{}", board::render(&snap));
}
