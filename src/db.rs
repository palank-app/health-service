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

/// Every row of the settings table, as a key to value map.
pub fn settings() -> impl std::future::Future<Output = Result<Settings, Error>> + Send {
    bridge(async move {
        #[derive(Deserialize)]
        struct Row {
            key: String,
            value: String,
        }
        let rows: Vec<Row> = config()
            .prepare("select key, value from settings")
            .all()
            .await
            .map_err(err)?
            .results()
            .map_err(err)?;
        Ok(Settings(rows.into_iter().map(|row| (row.key, row.value)).collect()))
    })
}

/// The settings table, read once per invocation.
pub struct Settings(std::collections::HashMap<String, String>);

impl Settings {
    fn get(&self, key: &str) -> &str {
        self.0.get(key).map_or("", String::as_str)
    }

    /// What the page calls itself.
    pub fn site_name(&self) -> &str {
        match self.get("site_name") {
            "" => "État des services",
            name => name,
        }
    }

    /// Where a state change is announced, when it is announced at all.
    /// Both addresses have to be there: the sender must sit on a domain
    /// the account routes email for, and the recipient must be verified.
    pub fn alert_addresses(&self) -> Option<(&str, &str)> {
        if self.get("email_alerts_enabled") != "1" {
            return None;
        }
        match (self.get("email_alert_sender"), self.get("email_alert_recipient")) {
            ("", _) | (_, "") => None,
            (from, to) => Some((from, to)),
        }
    }
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

/// Whether the last probe of a target answered as expected, or nothing
/// when it has never been probed.
pub fn last_state(slug: &str) -> impl std::future::Future<Output = Result<Option<bool>, Error>> + Send {
    let args = vec![s(slug)];
    bridge(async move {
        #[derive(Deserialize)]
        struct Row {
            ok: i64,
        }
        let found: Option<Row> = history()
            .prepare("select ok from checks where slug = ?1 order by at desc limit 1")
            .bind(&args)
            .map_err(err)?
            .first(None)
            .await
            .map_err(err)?;
        Ok(found.map(|row| row.ok == 1))
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
