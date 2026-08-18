-- What an announcement signs itself with: the display name beside the
-- email address, and the author of a webhook post.
--
--   wrangler d1 execute CONFIG --remote --file migrations/config/004_sender_name.sql

insert or ignore into settings (key, value) values ('alert_sender_name', 'Supervision');
