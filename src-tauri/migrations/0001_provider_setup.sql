CREATE TABLE IF NOT EXISTS provider_setup (
    provider_id TEXT PRIMARY KEY NOT NULL CHECK (provider_id <> ''),
    provider_user_id TEXT NOT NULL CHECK (provider_user_id <> ''),
    provider_api_key_fingerprint TEXT NOT NULL CHECK (provider_api_key_fingerprint <> ''),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
