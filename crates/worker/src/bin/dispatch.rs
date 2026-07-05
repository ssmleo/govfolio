//! Outbox alert dispatcher (design §6.3, goal 030): polls `outbox_event`,
//! matches against alert rules through the shared grammar, fans out email +
//! HMAC-signed webhooks with exactly-once dedup, retries and a DLQ.
//!
//! Usage:
//!   cargo run -p worker --bin dispatch -- \
//!     [--once] [--digest] [--interval-secs N] [--batch N]
//!
//! `--once` runs a single pass (tests/cron); `--digest` additionally runs the
//! digest pass (cadence = whoever invokes the flag — Cloud Scheduler later).
//!
//! Env: `DATABASE_URL` (required); `PUBLIC_BASE_URL` (provenance links,
//! default `<https://govfolio.io>`); `SMTP_HOST`/`SMTP_PORT`/`SMTP_USERNAME`/
//! `SMTP_PASSWORD`/`SMTP_FROM` (email channel — without them email
//! deliveries fail loudly into the DLQ; no creds live in code).

use anyhow::Context as _;

use worker::alerts::email::{EmailSender, SmtpConfig, SmtpEmailSender, SmtpUnconfigured};
use worker::alerts::matcher::match_pass;
use worker::alerts::sender::{digest_pass, send_pass};
use worker::alerts::webhook::HttpWebhookTransport;
use worker::alerts::{DispatchConfig, Senders};

struct Args {
    once: bool,
    digest: bool,
    interval: std::time::Duration,
    batch: i64,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = Args {
        once: false,
        digest: false,
        interval: std::time::Duration::from_secs(30),
        batch: 100,
    };
    let mut cli = std::env::args().skip(1);
    while let Some(flag) = cli.next() {
        match flag.as_str() {
            "--once" => args.once = true,
            "--digest" => args.digest = true,
            "--interval-secs" => {
                let raw = cli.next().context("--interval-secs requires a value")?;
                args.interval =
                    std::time::Duration::from_secs(raw.parse().context("--interval-secs")?);
            }
            "--batch" => {
                let raw = cli.next().context("--batch requires a value")?;
                args.batch = raw.parse().context("--batch")?;
            }
            other => anyhow::bail!(
                "unknown argument {other:?} (expected --once/--digest/--interval-secs/--batch)"
            ),
        }
    }
    Ok(args)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = parse_args()?;
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;

    let config = DispatchConfig {
        batch: args.batch,
        public_base_url: std::env::var("PUBLIC_BASE_URL")
            .unwrap_or_else(|_| "https://govfolio.io".to_owned()),
        ..DispatchConfig::default()
    };

    let email: Box<dyn EmailSender> = if let Some(smtp) = SmtpConfig::from_env()? {
        Box::new(SmtpEmailSender::new(&smtp)?)
    } else {
        eprintln!("SMTP not configured — email deliveries will dead-letter (set SMTP_* to enable)");
        Box::new(SmtpUnconfigured)
    };
    let webhook = HttpWebhookTransport::new()?;
    let senders = Senders {
        email: email.as_ref(),
        webhook: &webhook,
    };

    loop {
        let matched = match_pass(&pool, &config).await?;
        let sent = send_pass(&pool, &config, &senders).await?;
        print!(
            "matched: {} event(s) -> {} delivery(ies) | instant: {} sent, {} dead",
            matched.events, matched.deliveries, sent.sent, sent.dead
        );
        if args.digest {
            let digest = digest_pass(&pool, &config, &senders).await?;
            print!(" | digest: {} sent, {} dead", digest.sent, digest.dead);
        }
        println!();
        if args.once {
            return Ok(());
        }
        tokio::time::sleep(args.interval).await;
    }
}
