//! A status page that runs nowhere: the pages, the prober and the store
//! are one Worker, its two D1 bindings and a cron trigger.

pub mod alert;
pub mod app;
pub mod db;
pub mod probe;

use http_body_util::BodyExt;
use topcoat::router::{Body, Router, RouterBuilderDiscoverExt};
use worker::{
    console_log, event, Context, Env, Error, Headers, Request, Response, Result, ScheduleContext,
    ScheduledEvent,
};

/// Probes older than this are dropped on each sweep.
const KEEP_DAYS: i64 = 30;

/// The whole page as a pure function: worker::Request in, the router's
/// handle(), worker::Response out. No server underneath.
#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();
    db::install(&env).map_err(|e| Error::RustError(e.to_string()))?;

    let url = req.url()?;
    let target = format!(
        "{}{}",
        url.path(),
        url.query().map(|q| format!("?{q}")).unwrap_or_default()
    );
    let mut request = http::Request::builder().method(req.method().as_ref()).uri(target);
    for (name, value) in req.headers().entries() {
        request = request.header(&name, &value);
    }
    let body = req.clone()?.bytes().await?;
    let request = request.body(Body::from(body)).map_err(|e| Error::RustError(e.to_string()))?;

    let router = Router::builder().discover().build();
    let response = router.handle(request).await;

    let (head, body) = response.into_parts();
    let bytes = body.collect().await.map_err(|e| Error::RustError(e.to_string()))?.to_bytes();
    let headers = Headers::new();
    for (name, value) in head.headers.iter() {
        if let Ok(v) = value.to_str() {
            headers.append(name.as_str(), v)?;
        }
    }
    Ok(Response::from_bytes(bytes.to_vec())?
        .with_status(head.status.as_u16())
        .with_headers(headers))
}

/// One sweep per cron tick. A scheduled event has no request to answer, so
/// a failure only has the log to land in.
#[event(scheduled)]
async fn tick(event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    console_error_panic_hook::set_once();
    if let Err(e) = db::install(&env) {
        console_log!("cron {}: {e}", event.cron());
        return;
    }

    let settings = match db::settings().await {
        Ok(settings) => settings,
        Err(e) => {
            console_log!("cron {}: settings unreadable: {e}", event.cron());
            return;
        }
    };
    // An absent binding is not a failure: a deployment that never wants an
    // email simply does not declare one.
    let email = env.send_email("EMAIL").ok();

    match probe::sweep(&settings, email.as_ref()).await {
        Ok((healthy, probed)) => console_log!("cron {}: {healthy}/{probed} up", event.cron()),
        Err(e) => console_log!("cron {}: sweep failed: {e}", event.cron()),
    }

    if let Err(e) = db::prune(KEEP_DAYS).await {
        console_log!("cron {}: prune failed: {e}", event.cron());
    }
}
