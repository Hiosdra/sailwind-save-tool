CREATE TABLE IF NOT EXISTS snapshots (
    id TEXT PRIMARY KEY NOT NULL,
    slot INTEGER NOT NULL,
    created_at_utc TEXT NOT NULL,
    file_count INTEGER NOT NULL,
    total_size INTEGER NOT NULL,
    snapshot_path TEXT NOT NULL UNIQUE
);

CREATE INDEX IF NOT EXISTS snapshots_slot_created
ON snapshots(slot, created_at_utc DESC);
