-- Speakeasy Chat & Dateien Schema v3
-- Migration: Chat-Nachrichten und Dateiverwaltung

-- Chat-Nachrichten
CREATE TABLE IF NOT EXISTS chat_messages (
    id          TEXT PRIMARY KEY NOT NULL,
    channel_id  TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    sender_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    content     TEXT NOT NULL,
    message_type TEXT NOT NULL DEFAULT 'text' CHECK(message_type IN ('text', 'file', 'system')),
    reply_to    TEXT REFERENCES chat_messages(id) ON DELETE SET NULL,
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    edited_at   TEXT,
    deleted_at  TEXT
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_channel_id ON chat_messages(channel_id);
CREATE INDEX IF NOT EXISTS idx_chat_messages_sender_id ON chat_messages(sender_id);
CREATE INDEX IF NOT EXISTS idx_chat_messages_created_at ON chat_messages(created_at);
CREATE INDEX IF NOT EXISTS idx_chat_messages_deleted_at ON chat_messages(deleted_at);

-- Dateien
CREATE TABLE IF NOT EXISTS files (
    id          TEXT PRIMARY KEY NOT NULL,
    channel_id  TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    uploader_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    filename    TEXT NOT NULL,
    mime_type   TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    storage_path TEXT NOT NULL,
    checksum    TEXT NOT NULL, -- SHA-256
    created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    deleted_at  TEXT
);

CREATE INDEX IF NOT EXISTS idx_files_channel_id ON files(channel_id);
CREATE INDEX IF NOT EXISTS idx_files_uploader_id ON files(uploader_id);
CREATE INDEX IF NOT EXISTS idx_files_deleted_at ON files(deleted_at);

-- Datei-Kontingente pro Gruppe
CREATE TABLE IF NOT EXISTS file_quotas (
    group_id            TEXT NOT NULL PRIMARY KEY,
    max_file_size       INTEGER NOT NULL DEFAULT 10485760,    -- 10 MB
    max_total_storage   INTEGER NOT NULL DEFAULT 1073741824,  -- 1 GB
    current_usage       INTEGER NOT NULL DEFAULT 0
);
