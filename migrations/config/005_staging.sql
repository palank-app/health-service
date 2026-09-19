-- Watch staging beside prod, right under it.
--
--   wrangler d1 execute status-config --remote --file migrations/config/005_staging.sql

insert into targets (slug, name, url, expects, watched, rank) values
    ('app-staging', 'Application Palank (staging)', 'https://staging.palank.fr/api/v1/health', 200, 1, 15)
on conflict(slug) do update set
    url = excluded.url,
    expects = excluded.expects,
    watched = excluded.watched;
