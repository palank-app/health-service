# health-service

A public status page. Every watched endpoint is probed from the Cloudflare
network on a cron trigger, and the page shows what came back. The pages, the
prober and the store are one Worker with two D1 bindings.

- `GET /` — the page
- `GET /api/status` — the same summary as JSON

## Two databases

| Binding | Holds |
|---|---|
| `CONFIG` | `targets`: what to watch |
| `DB` | `checks`: what watching produced |

The target list is read, never written: a target dropped from the
configuration stops being probed and keeps its history. Apply
`migrations/config/001_targets.sql` to whichever database holds the
configuration, then fill it:

```sql
insert into targets (slug, name, url, expects, rank) values
    ('api', 'API', 'https://api.example.com/health', 200, 10);
```

`expects` is the status a healthy answer carries, so a service that answers
401 without a token is declared healthy without pretending it returns 200.

## Running it

```sh
cargo install worker-build            # once
wrangler d1 execute DB --local --file migrations/001_schema.sql
wrangler d1 execute CONFIG --local --file migrations/config/001_targets.sql
wrangler dev --test-scheduled
```

Nothing is probed until a sweep fires; `/__scheduled` fires one.

## Deploying

```sh
wrangler d1 create health-service     # paste the id into wrangler.toml
wrangler d1 execute DB --remote --file migrations/001_schema.sql
wrangler deploy
```

Point `CONFIG` at the configuration database in `wrangler.toml` and set
`site_name` in its `settings` table. The cron sweeps every five minutes.

## Alerts

A target that changes state is announced once, on whichever of the two
channels is configured; neither is required. Both sign with
`alert_sender_name`.

The webhook takes the payload shape Slack introduced, which Mattermost and
Rocket.Chat accept unchanged. Its URL is a credential, so it lives as a
secret rather than a setting:

```sh
wrangler secret put ALERT_WEBHOOK
```

Email goes through the `EMAIL` binding, which needs a paid Workers plan, a
sender on a domain the account routes mail for, and a verified recipient:

```sql
update settings set value = '1'                   where key = 'email_alerts_enabled';
update settings set value = 'status@example.com'  where key = 'email_alert_sender';
update settings set value = 'ops@example.com'     where key = 'email_alert_recipient';
```

## Limits

- Fifty subrequests per invocation on the free plan, one per target: the
  ceiling on how many targets a sweep can carry.
- Probes older than thirty days are dropped on each sweep.
- A failed probe records that it failed, not why.
