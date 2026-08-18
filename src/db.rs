//! The two stores: `CONFIG` says what to watch and what the page calls
//! itself, `DB` keeps what watching produced. Both are D1, spoken to
//! through the Worker bindings.
//!
//! The handles live in thread_locals because wasm has one thread and D1 is
//! not Send, while the router's handlers must be. Every call therefore runs
//! in a spawn_local task and the caller awaits a oneshot receiver, which is
//! Send and carries plain data.

use std::cell::RefCell;
use std::rc::Rc;

use serde::Deserialize;
use worker::wasm_bindgen::JsValue;
use worker::D1Database;

pub type Error = anyhow::Error;

/// A service to probe, as the configuration database describes it.
#[derive(Clone, Debug, Deserialize)]
pub struct Target {
    pub slug: String,
    pub name: String,
    pub url: String,
    pub expects: i64,
}

/// One probe, as it was recorded.
#[derive(Clone, Debug, Deserialize)]
pub struct Check {
    pub at: String,
    pub status: Option<i64>,
    pub latency_ms: i64,
    pub ok: i64,
}

thread_local! {
    static HISTORY: RefCell<Option<Rc<D1Database>>> = const { RefCell::new(None) };
    static CONFIG: RefCell<Option<Rc<D1Database>>> = const { RefCell::new(None) };
}

pub fn install(env: &worker::Env) -> Result<(), Error> {
    let history = env.d1("DB").map_err(|e| anyhow::anyhow!("DB binding: {e}"))?;
    let config = env.d1("CONFIG").map_err(|e| anyhow::anyhow!("CONFIG binding: {e}"))?;
    HISTORY.with(|cell| *cell.borrow_mut() = Some(Rc::new(history)));
    CONFIG.with(|cell| *cell.borrow_mut() = Some(Rc::new(config)));
    Ok(())
}

/// What the page calls itself, from the configuration database.
pub fn site_name() -> impl std::future::Future<Output = Result<String, Error>> + Send {
    bridge(async move {
        #[derive(Deserialize)]
        struct Row {
            value: String,
        }
        let found: Option<Row> = config()
            .prepare("select value from settings where key = 'site_name'")
            .first(None)
            .await
            .map_err(err)?;
        Ok(found.map_or_else(|| "État des services".to_string(), |row| row.value))
    })
}

fn history() -> Rc<D1Database> {
    HISTORY.with(|cell| cell.borrow().clone().expect("DB not installed"))
}

fn config() -> Rc<D1Database> {
    CONFIG.with(|cell| cell.borrow().clone().expect("CONFIG not installed"))
}

fn err(e: worker::Error) -> Error {
    anyhow::anyhow!("D1: {e}")
}

fn s(text: &str) -> JsValue {
    JsValue::from_str(text)
}

fn n(value: i64) -> JsValue {
    JsValue::from_f64(value as f64)
}

/// The Send bridge every call goes through.
fn bridge<T, F>(work: F) -> impl std::future::Future<Output = Result<T, Error>> + Send
where
    T: Send + 'static,
    F: std::future::Future<Output = Result<T, Error>> + 'static,
{
    let (tx, rx) = futures_channel::oneshot::channel();
    wasm_bindgen_futures::spawn_local(async move {
        let _ = tx.send(work.await);
    });
    async move { rx.await.map_err(|_| anyhow::anyhow!("bridge closed"))? }
}

/// Everything the configuration database says to watch, in display order.
pub fn targets() -> impl std::future::Future<Output = Result<Vec<Target>, Error>> + Send {
    bridge(async move {
        config()
            .prepare("select slug, name, url, expects from targets where watched = 1 order by rank, name")
            .all()
            .await
            .map_err(err)?
            .results::<Target>()
            .map_err(err)
    })
}

/// The last `how_many` probes of one target, newest first.
pub fn history_of(
    slug: &str,
    how_many: i64,
) -> impl std::future::Future<Output = Result<Vec<Check>, Error>> + Send {
    let args = vec![s(slug), n(how_many)];
    bridge(async move {
        history()
            .prepare(
                "select at, status, latency_ms, ok from checks \
                 where slug = ?1 order by at desc limit ?2",
            )
            .bind(&args)
            .map_err(err)?
            .all()
            .await
            .map_err(err)?
            .results::<Check>()
            .map_err(err)
    })
}

/// Writes one probe.
pub fn record(
    slug: &str,
    at: &str,
    status: Option<i64>,
    latency_ms: i64,
    ok: bool,
) -> impl std::future::Future<Output = Result<(), Error>> + Send {
    let args = vec![
        s(slug),
        s(at),
        status.map_or(JsValue::NULL, n),
        n(latency_ms),
        n(i64::from(ok)),
    ];
    bridge(async move {
        history()
            .prepare(
                "insert into checks (slug, at, status, latency_ms, ok) \
                 values (?1, ?2, ?3, ?4, ?5)",
            )
            .bind(&args)
            .map_err(err)?
            .run()
            .await
            .map_err(err)?;
        Ok(())
    })
}

/// Drops probes older than `keep_days`, so the history stays a page rather
/// than an archive.
pub fn prune(keep_days: i64) -> impl std::future::Future<Output = Result<u64, Error>> + Send {
    let args = vec![s(&format!("-{keep_days} days"))];
    bridge(async move {
        let out = history()
            .prepare("delete from checks where at < datetime('now', ?1)")
            .bind(&args)
            .map_err(err)?
            .run()
            .await
            .map_err(err)?;
        let meta = out.meta().map_err(err)?;
        Ok(meta.and_then(|m| m.changes).unwrap_or(0) as u64)
    })
}
