//! One sweep: every watched target is fetched once, what came back is
//! written down, and a target that changed state is announced. Called by
//! the scheduled event, never by a page.

use chrono::Utc;
use worker::console_log;
use worker::js_sys::Date;

use crate::alert::{Announcer, Change};
use crate::db;

/// Fetches every target and records the outcome. Returns how many answered
/// as expected, out of how many were probed.
///
/// There is no timeout of our own: the platform cuts a subrequest that
/// hangs, and a probe cut that way is recorded as down like any other.
pub async fn sweep(announcer: &Announcer) -> Result<(u32, u32), db::Error> {
    let targets = db::targets().await?;
    let at = Utc::now().to_rfc3339();
    let mut healthy = 0;
    let mut probed = 0;

    for target in targets {
        // One target's database trouble must not cost the others their probe.
        let recent = match db::history_of(&target.slug, 2).await {
            Ok(checks) => checks.iter().map(|check| check.ok == 1).collect::<Vec<bool>>(),
            Err(e) => {
                console_log!("state of {}: {e}", target.slug);
                continue;
            }
        };

        let started = Date::now();
        let (status, build) = fetch(&target.url).await;
        let latency_ms = (Date::now() - started).round() as i64;

        let ok = status == Some(target.expects);
        probed += 1;
        healthy += u32::from(ok);

        if let Err(e) = db::record(&target.slug, &at, status, latency_ms, ok, &build).await {
            console_log!("record for {}: {e}", target.slug);
            continue;
        }

        // Two consecutive failures before anything is said: a single
        // unreachable probe is usually the network. Recovery closes such a
        // run and only such a run, so a blip stays quiet in both directions.
        let change = match (recent.as_slice(), ok) {
            ([false, true], false) => {
                Some(Change::Down { name: &target.name, url: &target.url, status })
            }
            ([false, false], true) => {
                Some(Change::Recovered { name: &target.name, url: &target.url })
            }
            _ => None,
        };

        // A refused announcement must not cost the rest of the sweep: the
        // point of the pass is the record, the announcement is a courtesy.
        if let Some(change) = change {
            for failure in announcer.announce(&change).await {
                console_log!("alert for {}: {failure}", target.slug);
            }
        }
    }

    Ok((healthy, probed))
}

/// What a target said about itself: the build it runs and how long it has
/// been running. Absent for anything that is not a Palank health payload.
#[derive(Default)]
pub struct Build {
    pub sha: Option<String>,
    pub age: Option<i64>,
}

/// What one request came back with: the status, and what the body claimed.
/// The status is nothing when the request never came back.
async fn fetch(url: &str) -> (Option<i64>, Build) {
    let Some(response) = get(url).await else {
        return (None, Build::default());
    };
    let status = i64::from(response.status_code());
    (Some(status), read_build(response).await)
}

async fn get(url: &str) -> Option<worker::Response> {
    let mut request = worker::Request::new(url, worker::Method::Get).ok()?;
    request.headers_mut().ok()?.set("user-agent", "health-service").ok()?;
    // Redirects are followed: a service answering 301 towards its canonical
    // host is up.
    worker::Fetch::Request(request).send().await.ok()
}

/// A target may answer HTML, or JSON without these fields. Both read as an
/// absent build rather than a failed probe.
async fn read_build(mut response: worker::Response) -> Build {
    let Ok(body) = response.text().await else {
        return Build::default();
    };
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(&body) else {
        return Build::default();
    };
    Build {
        sha: payload["commit"].as_str().map(str::to_owned),
        age: payload["uptime"].as_i64(),
    }
}
