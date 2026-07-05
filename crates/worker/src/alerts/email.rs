//! Email channel: trait seam + the real SMTP sender (lettre, STARTTLS,
//! rustls/ring). Config-gated via `SMTP_*` env — hosts without creds build
//! [`SmtpUnconfigured`], whose sends fail terminally and land in the DLQ
//! (fail closed and visible; never silently "sent").

use anyhow::Context as _;
use async_trait::async_trait;
use lettre::message::Mailbox;
use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport as _, Message, Tokio1Executor};

use crate::alerts::SendError;

/// One plain-text email.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailMessage {
    /// Destination address (from the rule's channel config).
    pub to: String,
    /// Subject line.
    pub subject: String,
    /// Plain-text body.
    pub text: String,
}

/// Sender seam: production speaks SMTP, tests capture messages.
#[async_trait]
pub trait EmailSender: Send + Sync {
    /// Sends one message.
    ///
    /// # Errors
    /// [`SendError`] with retry classification.
    async fn send(&self, message: &EmailMessage) -> anyhow::Result<()>;
}

/// SMTP connection settings, read from env.
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    /// Relay hostname (`SMTP_HOST`).
    pub host: String,
    /// STARTTLS port (`SMTP_PORT`, default 587).
    pub port: u16,
    /// Auth username (`SMTP_USERNAME`).
    pub username: String,
    /// Auth password (`SMTP_PASSWORD`).
    pub password: String,
    /// From address (`SMTP_FROM`).
    pub from: String,
}

impl SmtpConfig {
    /// The config gate: `None` when `SMTP_HOST` is unset (no creds on this
    /// host); once the host is set, the remaining variables are REQUIRED.
    ///
    /// # Errors
    /// `SMTP_HOST` set but a companion variable missing/invalid.
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let Ok(host) = std::env::var("SMTP_HOST") else {
            return Ok(None);
        };
        if host.trim().is_empty() {
            return Ok(None);
        }
        let required = |name: &str| {
            std::env::var(name).with_context(|| format!("SMTP_HOST is set but {name} is not"))
        };
        let port = match std::env::var("SMTP_PORT") {
            Ok(raw) => raw.parse().context("SMTP_PORT is not a port number")?,
            Err(_) => 587,
        };
        Ok(Some(Self {
            host,
            port,
            username: required("SMTP_USERNAME")?,
            password: required("SMTP_PASSWORD")?,
            from: required("SMTP_FROM")?,
        }))
    }
}

/// The real SMTP sender (STARTTLS relay).
pub struct SmtpEmailSender {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpEmailSender {
    /// Wires the transport from config. No connection is made until a send.
    ///
    /// # Errors
    /// Malformed relay host or from-address.
    pub fn new(config: &SmtpConfig) -> anyhow::Result<Self> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .with_context(|| format!("smtp relay {}", config.host))?
            .port(config.port)
            .credentials(Credentials::new(
                config.username.clone(),
                config.password.clone(),
            ))
            .build();
        let from = config
            .from
            .parse()
            .with_context(|| format!("SMTP_FROM {:?} is not a mailbox", config.from))?;
        Ok(Self { transport, from })
    }
}

#[async_trait]
impl EmailSender for SmtpEmailSender {
    async fn send(&self, message: &EmailMessage) -> anyhow::Result<()> {
        let to: Mailbox = message.to.parse().map_err(|e| SendError {
            // A malformed address never gets better — straight to the DLQ.
            retryable: false,
            message: format!("recipient {:?}: {e}", message.to),
        })?;
        let email = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(&message.subject)
            .header(ContentType::TEXT_PLAIN)
            .body(message.text.clone())
            .map_err(|e| SendError {
                retryable: false,
                message: format!("building message: {e}"),
            })?;
        self.transport.send(email).await.map_err(|e| SendError {
            // SMTP hiccups (greylisting, 4xx codes) deserve the retry budget.
            retryable: true,
            message: format!("smtp send failed: {e}"),
        })?;
        Ok(())
    }
}

/// The config-gate fallback: every send fails terminally with a loud reason,
/// so email deliveries surface in the DLQ instead of pretending.
pub struct SmtpUnconfigured;

#[async_trait]
impl EmailSender for SmtpUnconfigured {
    async fn send(&self, _message: &EmailMessage) -> anyhow::Result<()> {
        Err(SendError {
            retryable: false,
            message: "SMTP is not configured on this host (set SMTP_HOST, SMTP_PORT, \
                      SMTP_USERNAME, SMTP_PASSWORD, SMTP_FROM)"
                .to_owned(),
        }
        .into())
    }
}
