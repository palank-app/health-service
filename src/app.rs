//! The public page and the JSON behind it. Both read the same summary, so
//! what a machine polls and what a person sees cannot disagree.

use topcoat::context::Cx;
use topcoat::router::response::{IntoResponse, Response};
use topcoat::router::{page, route, Body};
use topcoat::view::{component, view};
use topcoat::Result;

use crate::db::{self, Check};

/// How many probes the strip shows. At one probe every five minutes, this
/// is a little over eight hours.
const STRIP: i64 = 100;

/// A target with its history folded into what the page shows.
struct Row {
    name: String,
    slug: String,
    /// Newest first, as stored.
    checks: Vec<Check>,
}

impl Row {
    fn up(&self) -> Option<bool> {
        self.checks.first().map(|c| c.ok == 1)
    }

    /// Share of probes that answered as expected, over the strip.
    fn uptime(&self) -> Option<f64> {
        if self.checks.is_empty() {
            return None;
        }
        let ok = self.checks.iter().filter(|c| c.ok == 1).count() as f64;
        Some(ok * 100.0 / self.checks.len() as f64)
    }

    fn latency(&self) -> Option<i64> {
        self.checks.first().map(|c| c.latency_ms)
    }
}

async fn summary() -> Result<Vec<Row>> {
    let mut rows = Vec::new();
    for target in db::targets().await? {
        rows.push(Row {
            name: target.name,
            checks: db::history_of(&target.slug, STRIP).await?,
            slug: target.slug,
        });
    }
    Ok(rows)
}

#[component]
async fn strip(checks: Vec<Check>) -> Result {
    // Oldest on the left, as a timeline reads.
    let bars: Vec<&Check> = checks.iter().rev().collect();
    view! {
        <div class="strip">
            for check in bars {
                <span class=(if check.ok == 1 { "bar up" } else { "bar down" })
                      title=(format!("{} — {} ms", check.at, check.latency_ms))></span>
            }
        </div>
    }
}

#[page("/")]
async fn status(_cx: &Cx) -> Result {
    let rows = summary().await?;
    let title = db::settings().await?.site_name().to_string();
    let all_up = rows.iter().all(|r| r.up() == Some(true));
    let any_known = rows.iter().any(|r| r.up().is_some());

    view! {
        <!DOCTYPE html>
        <html lang="fr">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>(&title)</title>
                <meta name="description" content="L'état des services, sondés toutes les cinq minutes.">
                <link rel="stylesheet" href="/_static/site.css">
            </head>
            <body>
                <main>
                    <h1>(&title)</h1>
                    <p class=(if all_up && any_known { "banner up" } else if any_known { "banner down" } else { "banner unknown" })>
                        if !any_known {
                            "Aucune sonde pour l'instant."
                        } else if all_up {
                            "Tous les services répondent."
                        } else {
                            "Un service ne répond pas comme attendu."
                        }
                    </p>

                    <ul class="services">
                        for row in rows {
                            <li class="service">
                                <div class="head">
                                    <span class=(match row.up() {
                                        Some(true) => "dot up",
                                        Some(false) => "dot down",
                                        None => "dot unknown",
                                    })></span>
                                    <span class="name">(&row.name)</span>
                                    <span class="numbers">
                                        if let Some(uptime) = row.uptime() {
                                            (format!("{uptime:.1} %"))
                                        }
                                        if let Some(latency) = row.latency() {
                                            " · " (format!("{latency} ms"))
                                        }
                                    </span>
                                </div>
                                strip(checks: row.checks)
                            </li>
                        }
                    </ul>

                    <p class="foot">
                        "Sondé toutes les cinq minutes depuis le réseau Cloudflare."
                    </p>
                </main>
            </body>
        </html>
    }
}

struct Json(String);

impl IntoResponse for Json {
    fn into_response(self, _cx: &Cx) -> Result<Response> {
        Ok(Response::builder()
            .header("Content-Type", "application/json; charset=utf-8")
            .header("Cache-Control", "public, max-age=30")
            .body(Body::from(self.0))?)
    }
}

#[derive(serde::Serialize)]
struct Reported {
    slug: String,
    name: String,
    up: Option<bool>,
    uptime: Option<f64>,
    latency_ms: Option<i64>,
}

/// The same summary, for whatever polls it.
#[route(GET "/api/status")]
async fn api(_cx: &Cx) -> Result<Json> {
    let services: Vec<Reported> = summary()
        .await?
        .into_iter()
        .map(|row| Reported {
            up: row.up(),
            uptime: row.uptime().map(|u| (u * 10.0).round() / 10.0),
            latency_ms: row.latency(),
            slug: row.slug,
            name: row.name,
        })
        .collect();
    Ok(Json(serde_json::json!({ "services": services }).to_string()))
}
