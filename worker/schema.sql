-- Recall telemetry: one row per install per calendar day.
--
-- Apply with:  npm run db:init          (remote D1)
--              npm run db:init:local    (local dev D1)
--
-- `payload` is the full JSON summary the app sent (see src-tauri/src/telemetry.rs
-- for the schema). The typed columns are copied out of it for cheap grouping.
-- Legacy schema-1 pings ({ id } only) land here too, with NULL typed columns.

CREATE TABLE IF NOT EXISTS pings (
  install_id  TEXT NOT NULL,
  day         TEXT NOT NULL,                       -- YYYY-MM-DD, client-local
  received_at TEXT NOT NULL DEFAULT (datetime('now')),
  version     TEXT,
  os          TEXT,
  arch        TEXT,
  lang        TEXT,
  payload     TEXT NOT NULL,                       -- JSON
  PRIMARY KEY (install_id, day)
);

CREATE INDEX IF NOT EXISTS pings_day ON pings (day);
