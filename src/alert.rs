//! Announcing a state change, by email and by webhook.
//!
//! Email goes through the `EMAIL` binding, which needs a paid plan, a
//! sender on a domain the account routes mail for, and a recipient the
//! account has verified. A webhook needs none of that: it is one POST,
//! in the payload shape Slack introduced and Mattermost, Rocket.Chat and
//! others accept unchanged. Either channel can be absent; a deployment
//! that configures neither records its probes and says nothing.

use serde_json::json;
use worker::{EmailAddress, Env, Method, Request, RequestInit, SendEmail, SendEmailBuilder};

use crate::db::{Error, Settings};

/// What happened to one target.
pub enum Change<'a> {
    Down { name: &'a str, url: &'a str, status: Option<i64> },
    Recovered { name: &'a str, url: &'a str },
}

impl Change<'_> {
    fn subject(&self) -> String {
        match self {
            Change::Down { name, .. } => format!("{name} ne répond plus"),
            Change::Recovered { name, .. } => format!("{name} répond de nouveau"),
        }
    }

    fn detail(&self) -> String {
        match self {
            Change::Down { name, url, status } => {
                let answer = status
                    .map_or_else(|| "aucune réponse".to_string(), |code| format!("statut {code}"));
                format!("{name} ({url}) : {answer}.")
            }
            Change::Recovered { name, url } => format!("{name} ({url}) répond de nouveau."),
        }
    }

    /// Slack's palette, understood by everything that takes its payload.
    fn colour(&self) -> &'static str {
        match self {
            Change::Down { .. } => "#d24b4e",
            Change::Recovered { .. } => "#3db887",
        }
    }
}

/// The channels this deployment announces on, resolved once per sweep.
pub struct Announcer {
    email: Option<(SendEmail, String, String)>,
    webhook: Option<String>,
    /// What both channels sign with.
    from_name: String,
}

impl Announcer {
    /// Reads both channels. A missing binding, a missing secret or a
    /// setting left empty simply means that channel stays quiet.
    pub fn new(env: &Env, settings: &Settings) -> Self {
        let email = settings.alert_addresses().and_then(|(from, to)| {
            env.send_email("EMAIL").ok().map(|binding| {
                (binding, from.to_string(), to.to_string())
            })
        });
        let webhook = env.secret("ALERT_WEBHOOK").ok().map(|url| url.to_string());
        Self { email, webhook, from_name: settings.sender_name().to_string() }
    }

    pub fn is_silent(&self) -> bool {
        self.email.is_none() && self.webhook.is_none()
    }

    /// Announces on every configured channel, and reports what failed.
    /// One channel refusing does not stop the other.
    pub async fn announce(&self, change: &Change<'_>) -> Vec<Error> {
        let mut failures = Vec::new();
        if let Some((binding, from, to)) = &self.email
            && let Err(e) = send_email(binding, &self.from_name, from, to, change).await {
                failures.push(e);
            }
        if let Some(url) = &self.webhook
            && let Err(e) = post_webhook(url, &self.from_name, change).await {
                failures.push(e);
            }
        failures
    }
}

async fn send_email(
    binding: &SendEmail,
    from_name: &str,
    from: &str,
    to: &str,
    change: &Change<'_>,
) -> Result<(), Error> {
    let sender = EmailAddress::new(from_name, from);
    let message = SendEmailBuilder::builder_with_email_address_and_str(&sender, to, &change.subject())
        .text(&format!("{}\n", change.detail()))
        .build();

    binding
        .send_with_builder(&message)
        .await
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("email: {e:?}"))
}

/// One incoming webhook, in the Slack payload shape.
async fn post_webhook(url: &str, from_name: &str, change: &Change<'_>) -> Result<(), Error> {
    let payload = json!({
        "username": from_name,
        "attachments": [{
            "color": change.colour(),
            "fallback": change.subject(),
            "title": change.subject(),
            "text": change.detail(),
        }],
    });

    let mut init = RequestInit::new();
    init.with_method(Method::Post).with_body(Some(payload.to_string().into()));
    let request = Request::new_with_init(url, &init).map_err(|e| anyhow::anyhow!("webhook: {e}"))?;
    request
        .headers()
        .clone()
        .set("content-type", "application/json")
        .map_err(|e| anyhow::anyhow!("webhook: {e}"))?;

    let mut response =
        worker::Fetch::Request(request).send().await.map_err(|e| anyhow::anyhow!("webhook: {e}"))?;
    match response.status_code() {
        200..=299 => Ok(()),
        code => {
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!("webhook: {code} {}", body.trim()))
        }
    }
}
