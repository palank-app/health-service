# health-service

A public status page that runs as a Cloudflare Worker and nothing else: no
container, no Postgres, no host.

## Shape

- **D1, not a SQL server.** A Worker has no sockets, so sqlx cannot reach a
  database. Storage is D1 through the bindings, and the driver-specific
  code stays in `src/db.rs`.
- **Topcoat, not a web server.** `Router::handle` is a pure function, so
  the same crate renders the page inside the Worker with nothing under it.
- **Two databases.** `CONFIG` holds the target list and is only read here;
  `DB` holds the probe history. Nothing points across the two: D1 has no
  cross-database foreign key, and a target dropped from the configuration
  keeps its history.

Comments and identifiers in English; what the visitor reads is French.

## Watch out

- **The sweep runs on the cron trigger only.** There is no timer inside a
  Worker, and a page load never probes anything.
- **A probe is one subrequest**, capped at fifty per invocation on the free
  plan.
- **`wrangler dev` needs `--test-scheduled`** to expose `/__scheduled`,
  which is the only way to fire a sweep by hand.
- **An `if` around an `await` inside a topcoat handler compiles to invalid
  JavaScript** — a plain arrow holding an await. It fails at hydration with
  a SyntaxError and takes the rest of the page's hydration with it. This
  page has no handlers today; keep it that way or check the console.
