-- What the deployment calls itself, kept beside what it watches: the
-- repository stays generic, and one database describes one status page.
--
--   wrangler d1 execute CONFIG --remote --file migrations/config/002_settings.sql

create table if not exists settings (
    key   text primary key,
    value text not null
);

-- The heading and the browser tab. Absent, the page falls back to a
-- neutral title.
insert or ignore into settings (key, value) values ('site_name', 'État des services');
