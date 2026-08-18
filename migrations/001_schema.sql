-- What the probes produced. The targets live in the configuration
-- database, so a check carries the slug it was probed under and nothing
-- points across the two: D1 has no cross-database foreign key, and the
-- history of a target that config drops is worth keeping anyway.

create table checks (
    id         integer primary key autoincrement,
    slug       text    not null,
    at         text    not null,
    -- Null when the request never came back: a timeout or a refused
    -- connection has no status to report.
    status     integer,
    latency_ms integer not null,
    ok         integer not null
);

create index checks_by_target on checks (slug, at desc);
