-- The contract with the configuration database, applied there rather than
-- here. This service only reads the table; whoever owns the configuration
-- is free to fill it, and a target that disappears simply stops being
-- probed.
--
--   wrangler d1 execute CONFIG --remote --file migrations/config/001_targets.sql

create table if not exists targets (
    slug     text    primary key,
    name     text    not null,
    url      text    not null,
    -- The status a healthy answer carries. Most services say 200; one that
    -- answers 401 without a token is still up, and says so here.
    expects  integer not null default 200,
    -- A target left in the table but no longer probed keeps its history.
    watched  integer not null default 1,
    -- Display order on the page, low to high.
    rank     integer not null default 0
);

-- One row per endpoint to watch:
--
--   insert into targets (slug, name, url, expects, rank) values
--       ('api', 'API', 'https://api.example.com/health', 200, 10);
