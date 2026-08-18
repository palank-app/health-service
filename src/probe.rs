//! One sweep: every watched target is fetched once, what came back is
//! written down, and a target that changed state is announced. Called by
//! the scheduled event, never by a page.

use chrono::Utc;
use worker::js_sys::Date;
use worker::SendEmail;

use crate::alert::{self, Change};
use crate::db::{self, Settings};

/// Fetches every target and records the outcome. Returns how many answered
/// as expected, out of how many were probed.
///
/// There is no timeout of our own: the platform cuts a subrequest that
/// hangs, and a probe cut that way is recorded as down like any other.
pub async fn sweep(
    settings: &Settings,
    email: Option<&SendEmail>,
) -> Result<(u32, u32), db::Error> {
    let targets = db::targets().await?;
    let at = Utc::now().to_rfc3339();
    let mut healthy = 0;
    let mut probed = 0;

    for target in targets {
        probed += 1;
        let before = db::last_state(&target.slug).await?;

        let started = Date::now();
        let status = fetch_status(&target.url).await;
        let latency_ms = (Date::now() - started).round() as i64;

        let ok = status == Some(target.expects);
        healthy += u32::from(ok);
        db::record(&target.slug, &at, status, latency_ms, ok).await?;

        // Only a change is worth an email: a service down since yesterday
        // should not write every five minutes. A target probed for the
        // first time announces nothing either.
        let change = match (before, ok) {
            (Some(true), false) => {
                Some(Change::Down { name: &target.name, url: &target.url, status })
            }
            (Some(false), true) => Some(Change::Recovered { name: &target.name, url: &target.url }),
            _ => None,
        };

        if let (Some(change), Some((from, to)), Some(binding)) =
            (change, settings.alert_addresses(), email)
        {
            alert::send(binding, from, to, change).await?;
        }
    }

    Ok((healthy, probed))
}

/// The status one request came back with, or nothing when it never did.
async fn fetch_status(url: &str) -> Option<i64> {
    let mut request = worker::Request::new(url, worker::Method::Get).ok()?;
    request.headers_mut().ok()?.set("user-agent", "health-service").ok()?;
    // Redirects are followed: a service answering 301 towards its canonical
    // host is up.
    let response = worker::Fetch::Request(request).send().await.ok()?;
    Some(i64::from(response.status_code()))
}
