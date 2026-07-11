//! Generation-fenced jurisdiction lease CLI for factory producers.
//!
//! Usage:
//!   jurisdiction-lease claimable --epoch <n|En>
//!   jurisdiction-lease claim --next --epoch <n|En> [--as <lane-id>]
//!   jurisdiction-lease claim --id <x> [--as <lane-id>]
//!   jurisdiction-lease renew --id <x> --generation <n> [--as <lane-id>]
//!   jurisdiction-lease abandon --id <x> --generation <n> [--as <lane-id>]
//!   jurisdiction-lease status
//!
//! Direct `advance` and `release` commands are retired and fail closed.
//! Producers submit immutable receipts through `govfolio-loop`; only receipt
//! apply advances phase or releases a terminal lease.

use anyhow::{Context as _, bail};
use sqlx::PgPool;
use worker::lease;

const USAGE: &str = "usage:
  jurisdiction-lease claimable --epoch <n|En>
  jurisdiction-lease claim --next --epoch <n|En> [--as <lane-id>]
  jurisdiction-lease claim --id <x> [--as <lane-id>]
  jurisdiction-lease renew --id <x> --generation <n> [--as <lane-id>]
  jurisdiction-lease abandon --id <x> --generation <n> [--as <lane-id>]
  jurisdiction-lease status
(--as defaults from GOVFOLIO_LANE_ID; DATABASE_URL required)";

fn usage_exit(message: &str) -> ! {
    eprintln!("jurisdiction-lease: {message}\n{USAGE}");
    std::process::exit(2);
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|argument| argument == flag)
        .map(|index| match args.get(index + 1) {
            Some(value) if !value.starts_with("--") => value.clone(),
            _ => usage_exit(&format!("{flag} needs a value")),
        })
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|argument| argument == flag)
}

fn identity(args: &[String]) -> String {
    flag_value(args, "--as")
        .or_else(|| std::env::var("GOVFOLIO_LANE_ID").ok())
        .filter(|identity| !identity.trim().is_empty())
        .unwrap_or_else(|| usage_exit("no lane identity: pass --as <id> or set GOVFOLIO_LANE_ID"))
}

fn parse_epoch(raw: &str) -> i16 {
    raw.trim_start_matches(['E', 'e'])
        .parse()
        .unwrap_or_else(|_| usage_exit(&format!("--epoch {raw:?} is not a number or En")))
}

fn parse_generation(raw: &str) -> i64 {
    raw.parse()
        .ok()
        .filter(|generation| *generation >= 0)
        .unwrap_or_else(|| usage_exit(&format!("--generation {raw:?} is not non-negative")))
}

fn required_flag(args: &[String], flag: &str, command: &str) -> String {
    flag_value(args, flag).unwrap_or_else(|| usage_exit(&format!("{command} needs {flag} <value>")))
}

fn print_claim(claim: &lease::Lease) {
    println!(
        "claimed id={} phase={} epoch={} generation={}",
        claim.id,
        claim.coverage_phase,
        claim
            .epoch
            .map_or_else(|| "none".to_owned(), |epoch| epoch.to_string()),
        claim.generation,
    );
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        usage_exit("missing subcommand");
    };
    let rest = &args[1..];

    if matches!(command, "advance" | "release") {
        bail!(
            "{command} is retired: commit locally and submit an immutable receipt with \
             `govfolio-loop submit-receipt <receipt.json>`"
        );
    }

    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;

    match command {
        "claimable" => {
            let epoch = flag_value(rest, "--epoch").map_or_else(
                || usage_exit("claimable needs --epoch <n|En>"),
                |value| parse_epoch(&value),
            );
            let me = std::env::var("GOVFOLIO_LANE_ID")
                .ok()
                .filter(|identity| !identity.trim().is_empty());
            let rows = lease::claimable_count(&pool, me.as_deref(), epoch).await?;
            if rows == 0 {
                println!("none");
                std::process::exit(1);
            }
            println!("claimable epoch={epoch} rows={rows}");
        }
        "claim" => {
            let me = identity(rest);
            let claimed = if has_flag(rest, "--next") {
                let epoch = flag_value(rest, "--epoch").map_or_else(
                    || usage_exit("claim --next needs --epoch <n|En>"),
                    |value| parse_epoch(&value),
                );
                lease::claim_next(&pool, &me, epoch).await?
            } else if let Some(id) = flag_value(rest, "--id") {
                lease::claim_id(&pool, &me, &id).await?
            } else {
                usage_exit("claim needs --next --epoch <n> or --id <x>");
            };
            if let Some(claim) = claimed {
                print_claim(&claim);
            } else {
                println!("none");
                std::process::exit(1);
            }
        }
        "renew" | "abandon" => {
            let me = identity(rest);
            let id = required_flag(rest, "--id", command);
            let generation = parse_generation(&required_flag(rest, "--generation", command));
            let changed = if command == "renew" {
                lease::renew(&pool, &me, &id, generation).await?
            } else {
                lease::abandon(&pool, &me, &id, generation).await?
            };
            if !changed {
                println!("stale-or-pending id={id} generation={generation}");
                std::process::exit(1);
            }
            println!("{command}ed id={id} generation={generation}");
        }
        "status" => {
            let leases = lease::status(&pool).await?;
            if leases.is_empty() {
                println!("no-leases");
            }
            let now = chrono::Utc::now();
            for held in &leases {
                let age_minutes = (now - held.claimed_at).num_minutes();
                println!(
                    "lease id={} by={} phase={} generation={} pending={} age_min={age_minutes}",
                    held.id,
                    held.claimed_by,
                    held.coverage_phase,
                    held.generation,
                    held.pending_integration_id.as_deref().unwrap_or("none"),
                );
            }
        }
        other => usage_exit(&format!("unknown subcommand {other:?}")),
    }
    Ok(())
}
