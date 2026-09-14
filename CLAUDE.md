# health-service

> ## House rules
>
> **English in the code.** Comments, doc comments, log and error messages,
> identifiers and test names are English. French in any of them is a bug:
> translate it. There are two exceptions, and only two.
>
> - **Copy a user reads stays French.** The product speaks French to its
>   users. Labels, buttons, toasts, page text, e-mail bodies and messages
>   returned to a client are French. Never translate them.
> - **French legal and administrative terms with no clean English
>   equivalent** stay French: `mandat`, `dirigeants`, `bâti`, `non bâti`,
>   `siège`, `personne morale`, `représentant`, `procédures collectives`,
>   `loi Hoguet`.
>
> A French word is not a domain term just because the feature is French.
> `veille` became `watch`, because English has the word.
>
> **A French word keeps its accents.** In comments, in strings and in data:
> `2e étage`, never `2e etage`. Stripping an accent damages the word. It
> does not translate it.
>
> **A commit message is pure ASCII.** An accent in a commit body gets the
> commit rejected.
>
> **No decoration in a comment.** No em dash, guillemet, arrow, box-drawing
> rule or emoji. Write `--`, `"` and `->`.
>
> **Write to ASD-STE100** (Simplified Technical English):
>
> - Simple present tense, active voice.
> - One idea per sentence, 20 words at most.
> - Common concrete words. One word keeps one meaning across the repo.
> - Keep the articles. No noun cluster longer than three words.
>
> **A comment carries the WHY.** One line by default. State the fact and what
> breaks without it. Delete, do not rewrite, a comment that restates the code,
> narrates history ("used to", "now"), carries a ticket id, or documents a
> function's callers. The reason for a change belongs in the commit message.

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
