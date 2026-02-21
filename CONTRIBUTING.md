# Beitragen zu Speakeasy

Vielen Dank für dein Interesse an Speakeasy! Hier findest du alles, was du für deinen Beitrag brauchst.

## Voraussetzungen

- **Rust** (stable, via [rustup](https://rustup.rs))
- **Node.js 22+** (via [mise](https://mise.jdx.dev))
- **pnpm 9+** (`npm install -g pnpm`)
- **Docker** (für lokale Server-Tests)

## Projekt aufsetzen

```bash
git clone https://github.com/7blacky7/speakeasy.git
cd speakeasy

# Server-Abhängigkeiten prüfen
cargo build --workspace

# Client-Abhängigkeiten installieren
cd client && pnpm install
```

## Entwicklung starten

### Server

```bash
# Alle Tests ausführen
cargo test --workspace

# Server im Dev-Modus starten
cargo run --bin speakeasy-server

# Linting
cargo clippy --workspace --all-targets
```

### Client (Tauri + SolidJS)

```bash
cd client

# Im Entwicklungsmodus starten (Hot Reload)
pnpm tauri-dev

# Nur Frontend bauen
pnpm build

# Tauri-Release-Build
pnpm tauri-build
```

### Mit Docker

```bash
# Server-Image bauen
docker build -f docker/Dockerfile -t speakeasy-server .

# Alles zusammen starten
docker compose -f docker/docker-compose.yml up
```

## Tests ausführen

```bash
# Server-Tests
cargo test --workspace

# Einzelne Crate testen
cargo test -p speakeasy-auth

# Mit Ausgabe
cargo test --workspace -- --nocapture
```

## Code-Stil

- **Rust:** `cargo fmt` vor jedem Commit. `cargo clippy` darf keine Fehler zeigen.
- **TypeScript/JavaScript:** Vite/TypeScript-Konfiguration beachten.
- Alle Kommentare und Dokumentation auf **Deutsch**.

## Commit-Konventionen

Wir nutzen [Conventional Commits](https://www.conventionalcommits.org/):

```
<typ>(<scope>): <kurze Beschreibung auf Deutsch>

[optionaler Body]

[optionaler Footer]
```

### Typen

| Typ | Verwendung |
|-----|-----------|
| `feat` | Neue Funktion |
| `fix` | Fehlerbehebung |
| `docs` | Dokumentationsänderungen |
| `refactor` | Code-Umstrukturierung ohne Funktionsänderung |
| `test` | Tests hinzufügen oder anpassen |
| `ci` | CI/CD-Änderungen |
| `chore` | Wartungsarbeiten |

### Scopes (Beispiele)

`server`, `client`, `audio`, `auth`, `protocol`, `chat`, `plugin`, `ci`, `docker`

### Beispiele

```
feat(audio): Noise-Gate-Schwellenwert konfigurierbar machen
fix(auth): Token-Ablauf korrekt behandeln
ci: GitHub Actions Release-Workflow hinzufügen
```

## Pull Request Workflow

1. Fork des Repositories erstellen
2. Feature-Branch anlegen: `git checkout -b feat/mein-feature`
3. Änderungen entwickeln und committen
4. Sicherstellen, dass alle Tests bestehen: `cargo test --workspace`
5. Pull Request gegen `main` öffnen
6. PR-Beschreibung ausfüllen (was, warum, wie getestet)

## Branch-Strategie

- `main` – stabiler Branch, CI muss grün sein
- Feature-Branches: `feat/<name>`
- Bugfix-Branches: `fix/<name>`
- Kein direktes Pushen auf `main`

## Fragen?

Öffne ein [GitHub Issue](https://github.com/7blacky7/speakeasy/issues) für Fragen oder Diskussionen.
