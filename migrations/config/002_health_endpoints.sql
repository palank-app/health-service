-- Point every target at a health endpoint instead of `/`.
--
--   wrangler d1 execute status-config --remote --file migrations/config/002_health_endpoints.sql
--
-- Until now the nine targets were probed on `/`, which proves that a front
-- serves HTML, not that the service behind it works. Every Palank service
-- now answers a health endpoint that checks what it owns, and returns 503
-- when a dependency is down. The prober reads the status code only, so a
-- degraded service finally reads red.
--
-- Two shapes, and the difference is deliberate:
--   - a console answers /health, which aggregates its own Postgres and the
--     health of the Rust backend it fronts;
--   - everything else answers /api/v1/health.

update targets set url = 'https://app.palank.fr/api/v1/health',   expects = 200 where slug = 'app';
update targets set url = 'https://sso.palank.fr/api/v1/health',   expects = 200 where slug = 'sso';
update targets set url = 'https://email.palank.fr/api/v1/health', expects = 200 where slug = 'email';
update targets set url = 'https://ai.palank.fr/api/v1/health',    expects = 200 where slug = 'ai';
update targets set url = 'https://stt.palank.fr/api/v1/health',   expects = 200 where slug = 'stt';
update targets set url = 'https://www.palank.fr/api/v1/health',   expects = 200 where slug = 'website';

-- The consoles. `palankir` also drops from 401 to 200: its /api/* proxy
-- demands a bearer token, which a prober does not carry, so the old target
-- recorded "up" from an authentication error. /health needs no token.
update targets set url = 'https://palankey.palank.fr/health', expects = 200 where slug = 'palankey';
update targets set url = 'https://paliance.palank.fr/health', expects = 200 where slug = 'paliance';
update targets set url = 'https://palankir.palank.fr/health', expects = 200 where slug = 'palankir';

-- Two services that answer but were never watched.
insert into targets (slug, name, url, expects, watched, rank) values
    ('ocr',  'Service de lecture de documents (OCR)', 'https://ocr.palank.fr/api/v1/health',  200, 1, 100),
    ('addr', 'Service d''adresses (BAN)',             'https://addr.palank.fr/api/v1/health', 200, 1, 110)
on conflict(slug) do update set
    url = excluded.url,
    expects = excluded.expects,
    watched = excluded.watched;
