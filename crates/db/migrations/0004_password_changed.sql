-- Speakeasy Migration v4
-- Fuegt password_changed Flag zur users-Tabelle hinzu
-- Wird beim ersten Login mit Admin/admin-Passwort verwendet

ALTER TABLE users ADD COLUMN password_changed INTEGER NOT NULL DEFAULT 0;

-- Bestehende Benutzer gelten als "Passwort bereits geaendert"
-- (nur neue Admin-Benutzer beim ersten Start sollen den Zwang erleben)
UPDATE users SET password_changed = 1;
