# Speakeasy - Design Document

**Datum:** 2026-02-21
**Projektname:** Speakeasy
**Beschreibung:** Open-Source Voice-Communication-Plattform (TeamSpeak-Alternative)

---

## 1. Architektur-Uebersicht

**Typ:** Modularer Monolith (ein Server-Binary, intern sauber getrennte Module)

### Tech-Stack

| Komponente | Technologie |
|-----------|-------------|
| Server | Rust + Tokio (async runtime) |
| Client | Tauri 2 + SolidJS |
| Audio-Codec | Opus ueber eigenes UDP-Protokoll |
| Verschluesselung | DTLS (Transport) / E2E (pro Kanal konfigurierbar) |
| Plugins | WASM/WASI (wasmtime Runtime) |
| Datenbank | SQLite (Default Single-Instance) / PostgreSQL (Multi-Instance) |
| API | REST (v1/) + gRPC (Protobuf) + TCP/TLS line-based (ServerQuery-Style) |
| Deployment | Docker (Server), Native Installer (Client) |

### Server-Architektur

```
                        STUN/TURN (extern, z.B. coturn)
                              |
    UDP (Voice)               |          TCP/TLS (Control)
    DTLS oder E2E             |
         |                    |               |
         v                    v               v
+---------------------------------------------------------------------+
|                      Speakeasy Server                                |
|                   (Rust/Tokio - Modularer Monolith)                  |
|                                                                      |
|  +------------------+  +-------------------+  +-------------------+  |
|  | Media Router     |  | Signaling /       |  | Auth & Permission |  |
|  | (Voice Forwarder)|  | Session Service   |  | Service           |  |
|  |                  |  |                   |  |                   |  |
|  | - Jitter Buffer  |  | - Login/Session   |  | - Roles/Groups    |  |
|  | - PLC / FEC      |  | - UDP Negotiation |  | - Channel Perms   |  |
|  | - Congestion Ctrl|  | - ICE/STUN/TURN   |  | - Bans/Passwords  |  |
|  | - Seq/Timestamps |  |   Koordination    |  | - Commander API   |  |
|  | - Bitrate Adapt. |  | - Handshake/Auth  |  | - Audit Log       |  |
|  +--------+---------+  +--------+----------+  +--------+----------+  |
|           |                     |                      |              |
|  +--------+---------+  +-------+----------+  +---------+----------+  |
|  | Text / File      |  | E2E Crypto       |  | Plugin Host        |  |
|  | Service          |  | Subsystem        |  | (Server: WASM/WASI)|  |
|  |                  |  |                  |  |                    |  |
|  | - Chat/Channels  |  | - Key Management |  | - Capability Model |  |
|  | - File Upload/DL |  | - Group Keys     |  | - Signierung/Trust |  |
|  | - History        |  | - Key Rotation   |  | - Sandbox          |  |
|  | - Storage Backend|  | - Revocation     |  | - Host-API (intern)|  |
|  |   (Disk/S3/MinIO)|  |                  |  |                    |  |
|  +--------+---------+  +--------+---------+  +---------+----------+  |
|           |                     |                      |              |
|  +--------+---------------------+----------------------+-----------+  |
|  |              Core Event Bus (tokio channels, intern)            |  |
|  |   + Distributed PubSub (PG NOTIFY / NATS bei Multi-Instance)   |  |
|  +------------------------------+----------------------------------+  |
|                                 |                                    |
|  +------------------------------+----------------------------------+  |
|  |  Persistent: SQLite/PG          Ephemeral: In-Memory            |  |
|  |  (Accounts, Roles, Config,      (Voice-State, Endpoints,        |  |
|  |   Bans, Audit Logs)             Jitter Stats, Presence)         |  |
|  +----------------------------------------------------------------+  |
|                                                                      |
|  +----------------------------------------------------------------+  |
|  |         Observability / Admin API (REST/gRPC)                  |  |
|  |         Metrics (RTT, packet loss, jitter, bitrate, CPU)       |  |
|  |         Logs, Rate Limits, Health Checks                       |  |
|  +----------------------------------------------------------------+  |
+----------------------------------------------------------------------+
```

### Client-Architektur

```
+----------------------------------------------------------------------+
|                      Speakeasy Client                                 |
|                     (Tauri 2 + SolidJS)                               |
|                                                                       |
|  +------------------+  +------------------+  +---------------------+  |
|  | Audio Engine     |  | Chat & Files     |  | Server Browser      |  |
|  | (Rust/cpal)      |  | UI (SolidJS)     |  | & Permissions UI    |  |
|  |                  |  |                  |  |                     |  |
|  | - Capture/Play   |  | - Channels       |  | - Connect/Bookmarks |  |
|  | - Opus Enc/Dec   |  | - Messages       |  | - Roles/Admin       |  |
|  | - Noise Gate     |  | - File DL/UL     |  | - Commander UI      |  |
|  | - VAD / PTT      |  |                  |  |                     |  |
|  |   (Hold/Toggle)  |  |                  |  |                     |  |
|  | - AGC            |  |                  |  |                     |  |
|  | - Echo Cancel.   |  |                  |  |                     |  |
|  | - Noise Suppr.   |  |                  |  |                     |  |
|  +------------------+  +------------------+  +---------------------+  |
|                                                                       |
|  +------------------+  +------------------------------------------+  |
|  | Client Plugin    |  | Audio Settings                           |  |
|  | Host (WASM)      |  |                                          |  |
|  |                  |  | Simple Mode:                             |  |
|  | - Game Overlay   |  |   - Presets (Sprache/Ausgewogen/Musik)   |  |
|  | - Hotkeys/RPC    |  |   - Preset zeigt read-only Codec-Werte   |  |
|  | - Capabilities   |  |   - Auto-Setup / Kalibrieren Button      |  |
|  |   (WASI FS: off) |  |   - PTT: Hold / Toggle                  |  |
|  |   (Net: Host-API)|  |   - Beschreibungen: Latenz <-> Qualitaet |  |
|  |   (UI: nur Client|  |                                          |  |
|  | - Signierung     |  | Expert Mode:                             |  |
|  |                  |  |   - Bitrate, Samplerate, Buffer          |  |
|  |                  |  |   - Opus Params (VOIP/Audio/Lowdelay)    |  |
|  |                  |  |   - FEC, DTX, Frame Size                 |  |
|  |                  |  |   - Mic: Mono default, Stereo experim.   |  |
|  |                  |  |   - Latenz-Breakdown anzeigen:           |  |
|  |                  |  |     Device + Opus + Jitter = Gesamt      |  |
|  |                  |  |   - Echo Cancel Hinweis (Lautsprecher)   |  |
|  |                  |  |   - Reset: Preset / Default              |  |
|  |                  |  |                                          |  |
|  |                  |  | Live Monitor:                            |  |
|  |                  |  |   - Mic Input / After Processing         |  |
|  |                  |  |   - Clip/Peak Indicator                  |  |
|  |                  |  |   - Noise Floor (dB)                     |  |
|  |                  |  |   - Latenz, Loss%, RTT                   |  |
|  +------------------+  +------------------------------------------+  |
+-----------------------------------------------------------------------+
```

### Verschluesselung: Transport vs E2E

- **DTLS (Transport):** Verschluesselung Client <-> Server. Server sieht Audio.
  Fuer Kanaele wo Moderation/Plugins Audio verarbeiten muessen.
- **E2E (Ende-zu-Ende):** Audio-Payload Client <-> Client verschluesselt.
  Server forwardet blind (SFU-Style).
  - Key Management pro Channel (Join/Leave/Key Rotation)
  - Gruppen-Schluessel: neuer User sieht keine alten Pakete
  - Leave triggert Key Revocation
  - Eigenes Subsystem, nicht nachtraeglich "drangebaut"

### NAT Traversal

- STUN/TURN als **externe Infrastruktur** (z.B. coturn)
- Speakeasy nutzt es, reimplementiert es nicht
- ICE-like Koordination im Signaling Service
- UDP Keepalive/Heartbeat fuer NAT-Mappings

---

## 2. Permission-System

### Hierarchie

```
Server Admin (Root)
  +-- Server Default Group (jeder User ohne explizite Gruppe)
  +-- Server Groups (additive, mehrere pro User moeglich)
  |     z.B. "Moderator" + "VIP" gleichzeitig
  +-- Channel Default Group (jeder der joint)
  +-- Channel Groups (genau eine pro User pro Channel)
  +-- Individual Permissions (Override pro User)
```

### Aufloesung

```
Individual > Channel Group > Channel Default > Server Groups > Server Default
Bei Konflikten auf gleicher Ebene: Deny > Grant > Skip
```

### Permission-Typen

```rust
enum PermissionValue {
    TriState(Grant | Deny | Skip),   // Bool-Rechte (kick, ban, etc.)
    IntLimit(i64),                    // Quotas, Max-Clients, Channel-Tiefe
    Scope(Vec<Target>),              // Whisper-Targets, Gruppen-Filter
}
```

### Berechtigungs-Kategorien

| Kategorie | Permissions |
|-----------|------------|
| Server | Bearbeiten, Gruppen verwalten, Max-User, Bans, Server stoppen |
| Channel | Erstellen/Bearbeiten/Loeschen, Passwort, Temp-Channel, Max-Clients |
| User | Kicken, Bannen, Muten, Verschieben, Fluestern, Beschwerden |
| Datei | Upload, Download, Loeschen, Quota verwalten |
| Plugin | Installieren/Aktivieren, Plugin-Permissions verwalten |

### Audit & Safety

- **Audit Log:** Wer hat wen gebannt, Rechte geaendert, Channel geloescht
- **2-Person-Rule (optional):** Fuer "Server stop" / "Root transfer"

### Invite Links (Capability-basiert)

- Join Server / Join Channel / Assigned Group
- Ablaufdatum + One-time-use + Revocation

---

## 3. Commander (ServerQuery-Aequivalent)

### Zugangsarten

1. **TCP/TLS line-based** (ServerQuery-Feeling, aber verschluesselt - kein Plaintext-Telnet)
2. **REST API** (`/v1/...`) mit Versionierung + Deprecation
3. **gRPC (Protobuf)** fuer High-Performance, backwards-compatible Schema

### Auth-Modell

- User/Pass nur zum Login -> Session Token (kurzlebig)
- API Tokens (langlebig) fuer Bots mit Scopes: `cmd:clientkick`, `cmd:permissionwrite`, etc.

### Sicherheit

- Rate Limits per-IP und per-Token
- Separate Limits fuer "expensive commands" (file list, log export)

### Plugin-Kommunikation

- WASM Plugins -> Host-API (intern, direkt)
- Externe Bots/Tools -> REST/gRPC

### Befehle (Auszug)

```
server info/edit/stop
channel list/create/edit/delete
client list/kick/ban/move/poke
permission list/add/remove
file list/upload/delete
log view/export
plugin list/install/enable/disable
```

---

## 4. Audio-Einstellungen

### Simple Mode (Default)

- **Geraetewahl:** Mikrofon + Ausgabe mit Dropdown
- **Spracherkennung:** PTT (Hold / Toggle) oder Voice Activation
- **Auto-Setup Button:** Ein Klick kalibriert Pegel, Noise Gate, VAD
- **Presets:** Sprache / Ausgewogen / Musik/HiFi / Sparsam
  - Preset zeigt read-only was sich aendert (Bitrate/Frame/FEC/DTX/Jitter)
- **Rauschunterdrueckung:** Schieberegler mit Erklaerung
- **Lautstaerke:** Mikrofon + Ausgabe mit Test-Button
- **Latenz-Anzeige:** Echtzeit, farbcodiert

### Expert Mode

- **Mikrofon:** Sample Rate, Buffer Size (mit Latenz-Erklaerung)
  - Channels: Mono default, Stereo nur als "Experimental"
- **Opus Codec:** Bitrate (6-510 kbps), Frame Size, Application (VOIP/Audio/Lowdelay mit Erklaerungen), FEC, DTX
- **Voice Processing:** Noise Gate, Noise Suppression, AGC, Echo Cancellation (Hinweis: "Nur bei Lautsprechern"), De-Esser
- **Netzwerk:** Jitter Buffer (Fix/Adaptiv/Aus), Live-Stats
- **Latenz-Breakdown:** Device Buffer + Opus Frame + Jitter Buffer = Gesamt
- **Reset:** "Auf Preset zuruecksetzen" + "Alles auf Default"

### Live Monitor (beide Modi)

- Mic Input vs. After Processing (Wellenform)
- Clip/Peak Indicator (rot bei Uebersteuerung)
- Noise Floor Anzeige (dB)
- Latenz, Packet Loss %, RTT, aktuelle Bitrate

---

## 5. Plugin-System

### Architektur

- **Server-Plugins:** WASM/WASI via wasmtime, Host-API (intern)
- **Client-Plugins:** WASM mit Native Bridge fuer Overlay/Hotkeys/Game-Integration

### Capability-Modell

| Capability | Default | Beschreibung |
|-----------|---------|-------------|
| WASI FS Access | OFF | Dateisystemzugriff |
| Netzwerk | Nur via Host-API | Kein direkter Socket-Zugriff |
| UI/Overlay | Nur Client-seitig | Kein Server-UI |
| Audio Stream | Read-only (mit Erlaubnis) | Fuer Proximity-Voice etc. |

### Sicherheit

- Signierung/Trust (Code Signing)
- Sandbox-Isolation (ein fehlerhaftes Plugin crasht nicht den Server)
- Permission-Abfrage beim Installieren

---

## 6. Datenbank-Strategie

### Setup-Wizard Entscheidung

- **Single Instance:** SQLite (Default, zero config, WAL-Mode)
  - Backup = eine Datei kopieren
- **Multi Instance:** PostgreSQL (empfohlen)
  - Shared State via PG LISTEN/NOTIFY oder NATS
  - Docker Compose mit separatem PG Container

### Repository Pattern

Abstraktion ueber Trait/Interface, sodass SQLite und PostgreSQL austauschbar sind.

### Datentrennung

- **Persistent (DB):** Accounts, Rollen, Rechte, Config, Bans, Audit Logs
- **Ephemeral (Memory):** Voice-State, UDP Endpoints, Jitter Stats, Presence

---

## 7. Deployment

### Server (Docker)

```yaml
# docker-compose.yml (Single Instance)
services:
  speakeasy:
    image: speakeasy/server:latest
    ports:
      - "9987:9987/udp"   # Voice
      - "30033:30033/tcp"  # File Transfer
      - "10011:10011/tcp"  # ServerQuery (TLS)
      - "8080:8080/tcp"    # REST/gRPC API
    volumes:
      - ./data:/data       # SQLite + Files
    environment:
      - SE_DB_TYPE=sqlite
      - SE_MAX_CLIENTS=512
```

### Client

- Native Installer: Linux (.deb, .rpm, AppImage), Windows (.msi), macOS (.dmg)
- Auto-Update via Tauri Updater

---

## 8. Nicht-funktionale Anforderungen

- **Ressourcenschonend:** Minimaler RAM/CPU-Verbrauch
- **Keine Paywall:** Max-User nur durch Admin begrenzt
- **Cross-Platform:** Linux, Windows, macOS (Server + Client)
- **Open Source:** Lizenz TBD (vermutlich AGPL oder MIT)
