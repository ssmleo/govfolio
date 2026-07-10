//! CLI over [`worker::lease`] — the atomic jurisdiction lease for parallel
//! loop lanes (goal 097; closes `docs/runbooks/parallel-factory.md`
//! pre-check 1). Factory lanes call this at workflow steps 3/6
//! (`agents/workflows/factory-lane.md`); `agents/monitor.sh` calls `status`.
//!
//! Usage:
//!   jurisdiction-lease claim --next --epoch <n|En> [--as <lane-id>]
//!   jurisdiction-lease claim --id <x> [--as <lane-id>]
//!   jurisdiction-lease advance --id <x> --to <phase> [--as <lane-id>]
//!   jurisdiction-lease release --id <x> [--advance <phase> | --block <reason>] [--as <lane-id>]
//!   jurisdiction-lease status
//!
//! `--as` defaults from `GOVFOLIO_LANE_ID`; neither present → usage error
//! (fail closed: an anonymous lease would break the shared "who's doing
//! what" board). `--epoch` accepts `2` or `E2`. Env: `DATABASE_URL`
//! (required; no auto-migrate — a missing schema errors loudly).
//!
//! Exit codes: 0 = done; 1 = nothing claimable / lease not held (the
//! epoch-gate "nonzero = look at this" convention); 2 = usage. First stdout
//! line is machine-readable (`claimed id=.. phase=.. epoch=..`, `none`,
//! `advanced id=.. to=..`, `released id=..`, `not-held id=..`,
//! `lease id=.. by=.. phase=.. age_min=..`, `no-leases`).

use anyhow::Context as _;
use sqlx::PgPool;
use worker::lease::{self, Disposition};

const USAGE: &str = "usage:
  jurisdiction-lease claim --next --epoch <n|En> [--as <lane-id>]
  jurisdiction-lease claim --id <x> [--as <lane-id>]
  jurisdiction-lease advance --id <x> --to <phase> [--as <lane-id>]
  jurisdiction-lease release --id <x> [--advance <phase> | --block <reason>] [--as <lane-id>]
  jurisdiction-lease status
(--as defaults from GOVFOLIO_LANE_ID; DATABASE_URL required)";

fn usage_exit(msg: &str) -> ! {
    eprintln!("jurisdiction-lease: {msg}\n{USAGE}");
    std::process::exit(2);
}

/// Pull the value following a `--flag`; `None` when the flag is absent.
fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .map(|i| match args.get(i + 1) {
            Some(v) if !v.starts_with("--") => v.clone(),
            _ => usage_exit(&format!("{flag} needs a value")),
        })
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

/// Lane identity: `--as` wins, then `GOVFOLIO_LANE_ID`. No identity = no
/// lease operations (fail closed).
fn identity(args: &[String]) -> String {
    flag_value(args, "--as")
        .or_else(|| std::env::var("GOVFOLIO_LANE_ID").ok())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| usage_exit("no lane identity: pass --as <id> or set GOVFOLIO_LANE_ID"))
}

/// `2` or `E2` → 2.
fn parse_epoch(raw: &str) -> i16 {
    raw.trim_start_matches(['E', 'e'])
        .parse()
        .unwrap_or_else(|_| usage_exit(&format!("--epoch {raw:?} is not a number or En")))
}

fn print_claim(lease: &lease::Lease) {
    println!(
        "claimed id={} phase={} epoch={}",
        lease.id,
        lease.coverage_phase,
        lease
            .epoch
            .map_or_else(|| "none".to_owned(), |e| e.to_string()),
    );
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        usage_exit("missing subcommand");
    };
    let rest = &args[1..];

    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;

    match command {
        "claim" => {
            let me = identity(rest);
            let claimed = if has_flag(rest, "--next") {
                let epoch = flag_value(rest, "--epoch").map_or_else(
                    || usage_exit("claim --next needs --epoch <n|En>"),
                    |e| parse_epoch(&e),
                );
                lease::claim_next(&pool, &me, epoch).await?
            } else if let Some(id) = flag_value(rest, "--id") {
                lease::claim_id(&pool, &me, &id).await?
            } else {
                usage_exit("claim needs --next --epoch <n> or --id <x>");
            };
            if let Some(l) = claimed {
                print_claim(&l);
            } else {
                println!("none");
                std::process::exit(1);
            }
        }
        "advance" => {
            let me = identity(rest);
            let id =
                flag_value(rest, "--id").unwrap_or_else(|| usage_exit("advance needs --id <x>"));
            let to = flag_value(rest, "--to")
                .unwrap_or_else(|| usage_exit("advance needs --to <phase>"));
            if lease::advance(&pool, &me, &id, &to).await? {
                println!("advanced id={id} to={to}");
            } else {
                println!("not-held id={id}");
                std::process::exit(1);
            }
        }
        "release" => {
            let me = identity(rest);
            let id =
                flag_value(rest, "--id").unwrap_or_else(|| usage_exit("release needs --id <x>"));
            let disposition = match (flag_value(rest, "--advance"), flag_value(rest, "--block")) {
                (Some(_), Some(_)) => usage_exit("release takes --advance OR --block, not both"),
                (Some(phase), None) => Disposition::Advance(phase),
                (None, Some(reason)) => Disposition::Block(reason),
                (None, None) => Disposition::Keep,
            };
            if lease::release(&pool, &me, &id, disposition).await? {
                println!("released id={id}");
            } else {
                println!("not-held id={id}");
                std::process::exit(1);
            }
        }
        "status" => {
            let live = lease::status(&pool).await?;
            if live.is_empty() {
                println!("no-leases");
            }
            let now = chrono::Utc::now();
            for l in &live {
                let age_min = (now - l.claimed_at).num_minutes();
                println!(
                    "lease id={} by={} phase={} age_min={age_min}",
                    l.id, l.claimed_by, l.coverage_phase
                );
            }
        }
        other => usage_exit(&format!("unknown subcommand {other:?}")),
    }
    Ok(())
}
