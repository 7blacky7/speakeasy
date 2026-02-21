-- Speakeasy Datenbank-Schema v1
-- Initial-Migration: Grundstruktur aller Tabellen

-- Benutzer
CREATE TABLE IF NOT EXISTS users (
    id          TEXT PRIMARY KEY NOT NULL,  -- UUID als Text
    username    TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    last_login  TEXT,
    is_active   INTEGER NOT NULL DEFAULT 1  -- 0=inaktiv, 1=aktiv
);

CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_is_active ON users(is_active);

-- Kanaele
CREATE TABLE IF NOT EXISTS channels (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    parent_id       TEXT REFERENCES channels(id) ON DELETE SET NULL,
    topic           TEXT,
    password_hash   TEXT,
    max_clients     INTEGER NOT NULL DEFAULT 0,  -- 0 = unbegrenzt
    is_default      INTEGER NOT NULL DEFAULT 0,
    sort_order      INTEGER NOT NULL DEFAULT 0,
    channel_type    TEXT NOT NULL DEFAULT 'voice' CHECK(channel_type IN ('voice', 'text')),
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_channels_parent_id ON channels(parent_id);
CREATE INDEX IF NOT EXISTS idx_channels_sort_order ON channels(sort_order);

-- Server-Gruppen (additive, mehrere pro User)
CREATE TABLE IF NOT EXISTS server_groups (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT UNIQUE NOT NULL,
    priority    INTEGER NOT NULL DEFAULT 0,
    is_default  INTEGER NOT NULL DEFAULT 0,
    permissions TEXT NOT NULL DEFAULT '{}'  -- JSON
);

-- Kanal-Gruppen (eine pro User pro Kanal)
CREATE TABLE IF NOT EXISTS channel_groups (
    id          TEXT PRIMARY KEY NOT NULL,
    name        TEXT UNIQUE NOT NULL,
    permissions TEXT NOT NULL DEFAULT '{}'  -- JSON
);

-- User <-> Server-Gruppen (many-to-many)
CREATE TABLE IF NOT EXISTS user_server_groups (
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    group_id    TEXT NOT NULL REFERENCES server_groups(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, group_id)
);

CREATE INDEX IF NOT EXISTS idx_user_server_groups_user_id ON user_server_groups(user_id);

-- User <-> Kanal-Gruppen (eine pro User pro Kanal)
CREATE TABLE IF NOT EXISTS user_channel_groups (
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    channel_id  TEXT NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    group_id    TEXT NOT NULL REFERENCES channel_groups(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, channel_id)
);

CREATE INDEX IF NOT EXISTS idx_user_channel_groups_user ON user_channel_groups(user_id, channel_id);

-- Berechtigungen (flexibles Permission-System)
-- target_type: 'user' | 'server_group' | 'channel_group' | 'server_default' | 'channel_default'
-- value_type: 'tri_state' | 'int_limit' | 'scope'
-- tri_state: NULL=skip, 1=grant, 0=deny
-- scope: JSON-Array von erlaubten Werten
CREATE TABLE IF NOT EXISTS permissions (
    id              TEXT PRIMARY KEY NOT NULL,
    target_type     TEXT NOT NULL,
    target_id       TEXT,           -- NULL fuer Server-Default
    permission_key  TEXT NOT NULL,
    value_type      TEXT NOT NULL DEFAULT 'tri_state' CHECK(value_type IN ('tri_state', 'int_limit', 'scope')),
    tri_state       INTEGER,        -- NULL=skip, 1=grant, 0=deny
    int_limit       INTEGER,        -- fuer IntLimit-Typ
    scope_json      TEXT,           -- JSON-Array fuer Scope-Typ
    channel_id      TEXT REFERENCES channels(id) ON DELETE CASCADE  -- fuer Channel-spezifische Perms
);

CREATE INDEX IF NOT EXISTS idx_permissions_target ON permissions(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_permissions_key ON permissions(permission_key);
CREATE INDEX IF NOT EXISTS idx_permissions_channel ON permissions(channel_id);

-- Bans
CREATE TABLE IF NOT EXISTS bans (
    id          TEXT PRIMARY KEY NOT NULL,
    user_id     TEXT REFERENCES users(id) ON DELETE SET NULL,  -- NULL wenn nach IP gebannt
    ip          TEXT,           -- IP-Adresse (optional)
    reason      TEXT NOT NULL DEFAULT '',
    banned_by   TEXT REFERENCES users(id) ON DELETE SET NULL,
    expires_at  TEXT,           -- NULL = permanent
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_bans_user_id ON bans(user_id);
CREATE INDEX IF NOT EXISTS idx_bans_ip ON bans(ip);
CREATE INDEX IF NOT EXISTS idx_bans_expires_at ON bans(expires_at);

-- Audit-Log
CREATE TABLE IF NOT EXISTS audit_log (
    id          TEXT PRIMARY KEY NOT NULL,
    actor_id    TEXT REFERENCES users(id) ON DELETE SET NULL,
    action      TEXT NOT NULL,
    target_type TEXT,
    target_id   TEXT,
    details_json TEXT NOT NULL DEFAULT '{}',
    timestamp   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_audit_log_actor ON audit_log(actor_id);
CREATE INDEX IF NOT EXISTS idx_audit_log_action ON audit_log(action);
CREATE INDEX IF NOT EXISTS idx_audit_log_timestamp ON audit_log(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_log_target ON audit_log(target_type, target_id);

-- Einladungen
CREATE TABLE IF NOT EXISTS invites (
    id              TEXT PRIMARY KEY NOT NULL,
    code            TEXT UNIQUE NOT NULL,
    channel_id      TEXT REFERENCES channels(id) ON DELETE CASCADE,
    assigned_group_id TEXT REFERENCES server_groups(id) ON DELETE SET NULL,
    max_uses        INTEGER NOT NULL DEFAULT 0,  -- 0 = unbegrenzt
    used_count      INTEGER NOT NULL DEFAULT 0,
    expires_at      TEXT,           -- NULL = nie
    created_by      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_invites_code ON invites(code);
CREATE INDEX IF NOT EXISTS idx_invites_created_by ON invites(created_by);
CREATE INDEX IF NOT EXISTS idx_invites_expires_at ON invites(expires_at);
