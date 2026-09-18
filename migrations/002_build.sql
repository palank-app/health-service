-- What the probed service said about itself, beside whether it answered.
--
--   wrangler d1 execute health-service --remote --file migrations/002_build.sql
--
-- Both are null for a target that answers something other than a Palank
-- health payload, and for every check recorded before these columns existed.

-- `sha`, not `commit`: SQLite reserves that word and every query would
-- have to quote it.
alter table checks add column sha text;

-- Seconds since the service process started. Not `uptime`: on the page and
-- in /api/status that word already means the share of probes that passed.
alter table checks add column age integer;
