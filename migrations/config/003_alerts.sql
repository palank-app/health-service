-- Who hears about a service changing state, and whether anyone does.
-- Three settings rather than a manifest binding: the addresses belong to
-- the deployment, not to the repository.
--
--   wrangler d1 execute CONFIG --remote --file migrations/config/003_alerts.sql

insert or ignore into settings (key, value) values
    -- '1' to send, anything else to stay quiet.
    ('email_alerts_enabled',  '0'),
    -- Where an alert goes. Must be a verified destination on the account.
    ('email_alert_recipient', ''),
    -- Who it comes from. Must sit on a domain the account routes email for.
    ('email_alert_sender',    '');
