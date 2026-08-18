# health-service

A public status page in the shape of openstatus: every watched endpoint is
probed from the Cloudflare network on a cron trigger, and the page shows
what came back. There is no server, no container and no host to keep
alive — the pages, the prober and the store are one Worker with two D1
bindings.

- `GET /` — the page
- `GET /api/status` — the same summary as JSON

## Two databases

| Binding | Holds | Owned by |
|---|---|---|
| `CONFIG` | `targets`: what to watch | whoever owns the configuration |
| `DB` | `checks`: what watching produced | this service |

The target list is read, never written, and a target that disappears from
the configuration stops being probed while its history stays. The shape
this service expects is `migrations/config/001_targets.sql`; apply it to
whichever database holds the configuration, then fill the table:

```sql
insert into targets (slug, name, url, expects, rank) values
    ('api', 'API', 'https://api.example.com/health', 200, 10);
```

`expects` is the status a healthy answer carries, so a service that
answers 401 without a token can be declared healthy honestly rather than
by pretending it returns 200.

## Running it

```sh
cargo install worker-build            # once
wrangler d1 execute DB --local --file migrations/001_schema.sql
wrangler d1 execute CONFIG --local --file migrations/config/001_targets.sql
wrangler dev --test-scheduled
```

The sweep runs on the cron only, so a fresh page is empty until one fires.
Fire one by hand with a visit to `/__scheduled`.

## Deploying

```sh
wrangler d1 create health-service     # paste the id into wrangler.toml
wrangler d1 execute DB --remote --file migrations/001_schema.sql
wrangler deploy
```

Point `CONFIG` at the configuration database in `wrangler.toml`, then fill
its `settings` table — `site_name` is what the page calls itself.
Cloudflare runs the sweep every five minutes.

## Alerts

A target that changes state — up to down, or back — is announced once. A
service down since yesterday stays quiet, and a target probed for the
first time announces nothing.

Two channels, either or both, and neither is required.

Both sign with `alert_sender_name` from the settings table.

**A webhook**, in the payload shape Slack introduced and Mattermost,
Rocket.Chat and others accept unchanged. The URL is a credential — anyone
holding it can post to the channel — so it lives as a secret rather than
in the settings table:

```sh
wrangler secret put ALERT_WEBHOOK
```

**Email**, through the `EMAIL` binding. It asks more of you:

```sql
update settings set value = '1'                   where key = 'email_alerts_enabled';
update settings set value = 'status@example.com'  where key = 'email_alert_sender';
update settings set value = 'ops@example.com'     where key = 'email_alert_recipient';
```

- **Sending email from a Worker needs a paid Workers plan.** On the free
  plan the binding is refused and the sweep logs the refusal.
- Cloudflare accepts a sender only on a domain the account routes email
  for, and a recipient it has verified.

With no channel configured the sweep records its probes and says so once
in the log.

## Choices worth knowing

- **No Tailwind.** Its CLI and fontsource pull in `ring`, which has no wasm
  target. One page does not need a build step: the stylesheet is a file
  under `public/`, served by Workers Assets.
- **Probes older than thirty days are dropped** on each sweep, in the same
  invocation.
- **A probe is one subrequest**, and the free plan allows fifty per
  invocation — the ceiling on how many targets one sweep can carry.
- **A failed probe records no status.** The page shows it as down; it does
  not yet say whether it was DNS, TLS or a timeout.
