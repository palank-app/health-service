//! Announcing a state change by email, through the `EMAIL` binding.
//!
//! Cloudflare only accepts a sender on a domain the account routes email
//! for, and a recipient it has verified; both addresses therefore live in
//! the configuration rather than here.

use worker::{EmailAddress, SendEmail, SendEmailBuilder};

use crate::db::Error;

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

    fn body(&self) -> String {
        match self {
            Change::Down { name, url, status } => {
                let answer = status
                    .map_or_else(|| "aucune réponse".to_string(), |code| format!("statut {code}"));
                format!("{name} ({url}) : {answer}.\n")
            }
            Change::Recovered { name, url } => format!("{name} ({url}) répond de nouveau.\n"),
        }
    }
}

/// Sends one alert. The caller decides whether there is anything to send.
pub async fn send(
    binding: &SendEmail,
    from: &str,
    to: &str,
    change: Change<'_>,
) -> Result<(), Error> {
    let sender = EmailAddress::new("Supervision", from);
    let message = SendEmailBuilder::builder_with_email_address_and_str(&sender, to, &change.subject())
        .text(&change.body())
        .build();

    binding
        .send_with_builder(&message)
        .await
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("email: {e:?}"))
}
