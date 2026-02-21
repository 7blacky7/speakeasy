-- Speakeasy Datenbank-Schema v2
-- API-Token-Tabelle fuer Bots und Commander-Clients

CREATE TABLE IF NOT EXISTS api_tokens (
    id              TEXT PRIMARY KEY NOT NULL,       -- UUID als Text
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    beschreibung    TEXT NOT NULL DEFAULT '',        -- Beschreibungstext
    scopes_json     TEXT NOT NULL DEFAULT '[]',     -- JSON-Array von Scope-Strings
    token_hash      TEXT NOT NULL,                  -- Argon2id-Hash des Token-Werts
    token_praefix   TEXT NOT NULL,                  -- Erste 8 Zeichen zur Anzeige
    erstellt_am     TEXT NOT NULL DEFAULT (datetime('now')),
    laeuft_ab_am    TEXT,                           -- NULL = nie ablaufend
    widerrufen      INTEGER NOT NULL DEFAULT 0      -- 0=aktiv, 1=widerrufen
);

CREATE INDEX IF NOT EXISTS idx_api_tokens_user_id ON api_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_api_tokens_widerrufen ON api_tokens(widerrufen);
CREATE INDEX IF NOT EXISTS idx_api_tokens_laeuft_ab_am ON api_tokens(laeuft_ab_am);
