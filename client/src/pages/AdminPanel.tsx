import { A } from "@solidjs/router";
import {
  createSignal,
  createEffect,
  onCleanup,
  Show,
  For,
  type JSX,
} from "solid-js";
import Modal from "../components/ui/Modal";
import {
  adminGetServer,
  adminGetClients,
  adminKickClient,
  adminBanClient,
  adminMoveClient,
  adminPokeClient,
  adminGetLogs,
  adminUpdateServer,
  type AdminClientInfo,
  type AdminServerInfo,
  type AuditLogEntry,
  type ChannelInfo,
  getServerInfo,
} from "../bridge";
import styles from "./AdminPanel.module.css";

// --- Tab-Definitionen ---

type TabId =
  | "overview"
  | "users"
  | "bans"
  | "settings"
  | "audit"
  | "invites";

const TABS: { id: TabId; label: string }[] = [
  { id: "overview", label: "Uebersicht" },
  { id: "users", label: "Benutzer" },
  { id: "bans", label: "Bans" },
  { id: "settings", label: "Einstellungen" },
  { id: "audit", label: "Audit-Log" },
  { id: "invites", label: "Einladungen" },
];

// --- Hilfsfunktionen ---

function formatTimestamp(ts: string | null | undefined): string {
  if (!ts) return "-";
  try {
    const d = new Date(ts);
    return d.toLocaleString("de-DE", {
      day: "2-digit",
      month: "2-digit",
      year: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  } catch {
    return ts;
  }
}

function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

function formatDuration(secs: number | null | undefined): string {
  if (secs === null || secs === undefined) return "Permanent";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

// =======================================================================
// Uebersicht-Tab
// =======================================================================

function OverviewTab() {
  const [info, setInfo] = createSignal<AdminServerInfo | null>(null);
  const [error, setError] = createSignal("");

  const load = async () => {
    try {
      setInfo(await adminGetServer());
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  createEffect(() => {
    load();
    const timer = setInterval(load, 10000);
    onCleanup(() => clearInterval(timer));
  });

  return (
    <div>
      <Show when={error()}>
        <div class={styles.errorText}>{error()}</div>
      </Show>
      <Show when={info()} fallback={<div class={styles.emptyState}>Lade Server-Informationen...</div>}>
        {(serverInfo) => (
          <div class={styles.infoGrid}>
            <div class={styles.infoCard}>
              <div class={styles.infoLabel}>Server-Name</div>
              <div class={styles.infoValue}>{serverInfo().name}</div>
            </div>
            <div class={styles.infoCard}>
              <div class={styles.infoLabel}>Version</div>
              <div class={styles.infoValue}>{serverInfo().version}</div>
            </div>
            <div class={styles.infoCard}>
              <div class={styles.infoLabel}>Uptime</div>
              <div class={styles.infoValue}>{formatUptime(serverInfo().uptime_secs)}</div>
            </div>
            <div class={styles.infoCard}>
              <div class={styles.infoLabel}>Online Clients</div>
              <div class={styles.infoValue}>{serverInfo().online_clients}</div>
            </div>
            <div class={styles.infoCard}>
              <div class={styles.infoLabel}>Max Clients</div>
              <div class={styles.infoValue}>{serverInfo().max_clients}</div>
            </div>
          </div>
        )}
      </Show>
    </div>
  );
}

// =======================================================================
// Benutzer-Tab
// =======================================================================

type UserDialog =
  | { type: "none" }
  | { type: "kick"; client: AdminClientInfo }
  | { type: "ban"; client: AdminClientInfo }
  | { type: "move"; client: AdminClientInfo }
  | { type: "poke"; client: AdminClientInfo };

function UsersTab() {
  const [clients, setClients] = createSignal<AdminClientInfo[]>([]);
  const [channels, setChannels] = createSignal<ChannelInfo[]>([]);
  const [error, setError] = createSignal("");
  const [dialog, setDialog] = createSignal<UserDialog>({ type: "none" });
  const [search, setSearch] = createSignal("");

  // Dialog-Felder
  const [reason, setReason] = createSignal("");
  const [banDuration, setBanDuration] = createSignal("");
  const [moveChannel, setMoveChannel] = createSignal("");
  const [pokeMessage, setPokeMessage] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [actionError, setActionError] = createSignal("");

  const load = async () => {
    try {
      const [clientList, serverInfo] = await Promise.all([
        adminGetClients(),
        getServerInfo(),
      ]);
      setClients(clientList);
      setChannels(serverInfo.channels);
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  createEffect(() => {
    load();
    const timer = setInterval(load, 5000);
    onCleanup(() => clearInterval(timer));
  });

  const filteredClients = () => {
    const s = search().toLowerCase();
    if (!s) return clients();
    return clients().filter(
      (c) =>
        c.username.toLowerCase().includes(s) ||
        (c.ip ?? "").includes(s) ||
        (c.channel_name ?? "").toLowerCase().includes(s)
    );
  };

  const openDialog = (type: UserDialog["type"], client: AdminClientInfo) => {
    setReason("");
    setBanDuration("");
    setMoveChannel("");
    setPokeMessage("");
    setActionError("");
    if (type === "none") return;
    setDialog({ type, client } as UserDialog);
  };

  const closeDialog = () => setDialog({ type: "none" });

  const doKick = async () => {
    const d = dialog();
    if (d.type !== "kick") return;
    setBusy(true);
    setActionError("");
    try {
      await adminKickClient(d.client.id, reason() || undefined);
      closeDialog();
      load();
    } catch (e) {
      setActionError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const doBan = async () => {
    const d = dialog();
    if (d.type !== "ban") return;
    setBusy(true);
    setActionError("");
    try {
      const dur = banDuration() ? parseInt(banDuration()) * 60 : undefined;
      await adminBanClient(d.client.id, dur, reason() || undefined);
      closeDialog();
      load();
    } catch (e) {
      setActionError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const doMove = async () => {
    const d = dialog();
    if (d.type !== "move") return;
    if (!moveChannel()) return;
    setBusy(true);
    setActionError("");
    try {
      await adminMoveClient(d.client.id, moveChannel());
      closeDialog();
      load();
    } catch (e) {
      setActionError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const doPoke = async () => {
    const d = dialog();
    if (d.type !== "poke") return;
    if (!pokeMessage()) return;
    setBusy(true);
    setActionError("");
    try {
      await adminPokeClient(d.client.id, pokeMessage());
      closeDialog();
    } catch (e) {
      setActionError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <Show when={error()}>
        <div class={styles.errorText}>{error()}</div>
      </Show>

      <div class={styles.toolbar}>
        <input
          class={styles.searchInput}
          type="text"
          placeholder="Benutzer suchen..."
          value={search()}
          onInput={(e) => setSearch(e.currentTarget.value)}
        />
        <span class={`${styles.badge} ${styles.badgeActive}`}>
          {clients().length} Online
        </span>
      </div>

      <Show
        when={filteredClients().length > 0}
        fallback={<div class={styles.emptyState}>Keine Clients verbunden</div>}
      >
        <table class={styles.table}>
          <thead>
            <tr>
              <th>Benutzer</th>
              <th>Channel</th>
              <th>IP</th>
              <th>Verbunden seit</th>
              <th>Aktionen</th>
            </tr>
          </thead>
          <tbody>
            <For each={filteredClients()}>
              {(client) => (
                <tr>
                  <td>{client.username}</td>
                  <td>{client.channel_name ?? "-"}</td>
                  <td class={styles.mono}>{client.ip ?? "-"}</td>
                  <td class={styles.mono}>
                    {formatTimestamp(client.connected_since)}
                  </td>
                  <td>
                    <div class={styles.actionGroup}>
                      <button
                        class={`${styles.actionBtn} ${styles.warnBtn}`}
                        onClick={() => openDialog("kick", client)}
                      >
                        Kick
                      </button>
                      <button
                        class={`${styles.actionBtn} ${styles.dangerBtn}`}
                        onClick={() => openDialog("ban", client)}
                      >
                        Ban
                      </button>
                      <button
                        class={styles.actionBtn}
                        onClick={() => openDialog("move", client)}
                      >
                        Verschieben
                      </button>
                      <button
                        class={styles.actionBtn}
                        onClick={() => openDialog("poke", client)}
                      >
                        Poke
                      </button>
                    </div>
                  </td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </Show>

      {/* Kick-Dialog */}
      <Show when={dialog().type === "kick"}>
        <Modal
          title={`Client kicken: ${(dialog() as { type: "kick"; client: AdminClientInfo }).client.username}`}
          onClose={closeDialog}
          actions={
            <>
              <button class={styles.actionBtn} onClick={closeDialog}>
                Abbrechen
              </button>
              <button
                class={`${styles.actionBtn} ${styles.dangerBtn}`}
                onClick={doKick}
                disabled={busy()}
              >
                {busy() ? "Kicke..." : "Kicken"}
              </button>
            </>
          }
        >
          <div class={styles.fieldRow}>
            <label class={styles.label}>Grund (optional)</label>
            <input
              class={styles.input}
              type="text"
              value={reason()}
              onInput={(e) => setReason(e.currentTarget.value)}
              placeholder="Grund fuer den Kick..."
            />
          </div>
          <Show when={actionError()}>
            <span class={styles.errorText}>{actionError()}</span>
          </Show>
        </Modal>
      </Show>

      {/* Ban-Dialog */}
      <Show when={dialog().type === "ban"}>
        <Modal
          title={`Client bannen: ${(dialog() as { type: "ban"; client: AdminClientInfo }).client.username}`}
          onClose={closeDialog}
          actions={
            <>
              <button class={styles.actionBtn} onClick={closeDialog}>
                Abbrechen
              </button>
              <button
                class={`${styles.actionBtn} ${styles.dangerBtn}`}
                onClick={doBan}
                disabled={busy()}
              >
                {busy() ? "Banne..." : "Bannen"}
              </button>
            </>
          }
        >
          <div class={styles.fieldRow}>
            <label class={styles.label}>Grund (optional)</label>
            <input
              class={styles.input}
              type="text"
              value={reason()}
              onInput={(e) => setReason(e.currentTarget.value)}
              placeholder="Grund fuer den Ban..."
            />
          </div>
          <div class={styles.fieldRow}>
            <label class={styles.label}>Dauer in Minuten (leer = permanent)</label>
            <input
              class={styles.input}
              type="number"
              min="1"
              value={banDuration()}
              onInput={(e) => setBanDuration(e.currentTarget.value)}
              placeholder="z.B. 60"
            />
          </div>
          <Show when={actionError()}>
            <span class={styles.errorText}>{actionError()}</span>
          </Show>
        </Modal>
      </Show>

      {/* Verschieben-Dialog */}
      <Show when={dialog().type === "move"}>
        <Modal
          title={`Client verschieben: ${(dialog() as { type: "move"; client: AdminClientInfo }).client.username}`}
          onClose={closeDialog}
          actions={
            <>
              <button class={styles.actionBtn} onClick={closeDialog}>
                Abbrechen
              </button>
              <button
                class={`${styles.actionBtn}`}
                onClick={doMove}
                disabled={busy() || !moveChannel()}
              >
                {busy() ? "Verschiebe..." : "Verschieben"}
              </button>
            </>
          }
        >
          <div class={styles.fieldRow}>
            <label class={styles.label}>Ziel-Channel</label>
            <select
              class={styles.filterSelect}
              value={moveChannel()}
              onChange={(e) => setMoveChannel(e.currentTarget.value)}
            >
              <option value="">-- Channel waehlen --</option>
              <For each={channels()}>
                {(ch) => <option value={ch.id}>{ch.name}</option>}
              </For>
            </select>
          </div>
          <Show when={actionError()}>
            <span class={styles.errorText}>{actionError()}</span>
          </Show>
        </Modal>
      </Show>

      {/* Poke-Dialog */}
      <Show when={dialog().type === "poke"}>
        <Modal
          title={`Nachricht senden an: ${(dialog() as { type: "poke"; client: AdminClientInfo }).client.username}`}
          onClose={closeDialog}
          actions={
            <>
              <button class={styles.actionBtn} onClick={closeDialog}>
                Abbrechen
              </button>
              <button
                class={styles.actionBtn}
                onClick={doPoke}
                disabled={busy() || !pokeMessage()}
              >
                {busy() ? "Sende..." : "Senden"}
              </button>
            </>
          }
        >
          <div class={styles.fieldRow}>
            <label class={styles.label}>Nachricht</label>
            <input
              class={styles.input}
              type="text"
              value={pokeMessage()}
              onInput={(e) => setPokeMessage(e.currentTarget.value)}
              placeholder="Nachricht an den Client..."
            />
          </div>
          <Show when={actionError()}>
            <span class={styles.errorText}>{actionError()}</span>
          </Show>
        </Modal>
      </Show>
    </div>
  );
}

// =======================================================================
// Bans-Tab
// =======================================================================

function BansTab() {
  // Ban-Verwaltung: Da die REST-API aktuell keinen eigenen /v1/bans Endpunkt hat,
  // zeigen wir Ban-Events aus dem Audit-Log an
  const [logs, setLogs] = createSignal<AuditLogEntry[]>([]);
  const [error, setError] = createSignal("");

  const load = async () => {
    try {
      const entries = await adminGetLogs(100, 0, "client.gebannt");
      setLogs(entries);
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  createEffect(() => {
    load();
  });

  return (
    <div>
      <Show when={error()}>
        <div class={styles.errorText}>{error()}</div>
      </Show>

      <div class={styles.toolbar}>
        <span class={styles.muted}>
          Ban-Eintraege aus dem Audit-Log (letzte 100)
        </span>
        <div class={styles.toolbarRight}>
          <button class={styles.actionBtn} onClick={load}>
            Aktualisieren
          </button>
        </div>
      </div>

      <Show
        when={logs().length > 0}
        fallback={<div class={styles.emptyState}>Keine Ban-Eintraege vorhanden</div>}
      >
        <table class={styles.table}>
          <thead>
            <tr>
              <th>Zeitpunkt</th>
              <th>Gebannter User</th>
              <th>Gebannt von</th>
              <th>Grund</th>
              <th>Dauer</th>
            </tr>
          </thead>
          <tbody>
            <For each={logs()}>
              {(entry) => (
                <tr>
                  <td class={styles.mono}>{formatTimestamp(entry.zeitstempel)}</td>
                  <td class={styles.mono}>{entry.ziel_id ?? "-"}</td>
                  <td class={styles.mono}>{entry.aktor_id ?? "-"}</td>
                  <td>{(entry.details as Record<string, string>)?.grund ?? "-"}</td>
                  <td>
                    <span
                      class={`${styles.badge} ${
                        !(entry.details as Record<string, unknown>)?.dauer_secs
                          ? styles.badgePermanent
                          : ""
                      }`}
                    >
                      {formatDuration(
                        (entry.details as Record<string, number>)?.dauer_secs ?? null
                      )}
                    </span>
                  </td>
                </tr>
              )}
            </For>
          </tbody>
        </table>
      </Show>
    </div>
  );
}

// =======================================================================
// Server-Einstellungen Tab
// =======================================================================

function SettingsTab() {
  const [name, setName] = createSignal("");
  const [welcome, setWelcome] = createSignal("");
  const [maxClients, setMaxClients] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  const [success, setSuccess] = createSignal("");
  const [loaded, setLoaded] = createSignal(false);

  const load = async () => {
    try {
      const info = await adminGetServer();
      setName(info.name);
      setMaxClients(String(info.max_clients));
      setLoaded(true);
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  createEffect(() => {
    load();
  });

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    setBusy(true);
    setError("");
    setSuccess("");
    try {
      await adminUpdateServer({
        name: name() || undefined,
        willkommensnachricht: welcome() || undefined,
        max_clients: maxClients() ? parseInt(maxClients()) : undefined,
      });
      setSuccess("Server-Einstellungen gespeichert");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <Show when={!loaded()}>
        <div class={styles.emptyState}>Lade Einstellungen...</div>
      </Show>
      <Show when={loaded()}>
        <form onSubmit={handleSubmit}>
          <div class={styles.section}>
            <div class={styles.sectionTitle}>Server-Konfiguration</div>
            <div class={styles.sectionBody}>
              <div class={styles.fieldRow}>
                <label class={styles.label}>Server-Name</label>
                <input
                  class={styles.input}
                  type="text"
                  value={name()}
                  onInput={(e) => setName(e.currentTarget.value)}
                  disabled={busy()}
                />
              </div>
              <div class={styles.fieldRow}>
                <label class={styles.label}>Willkommensnachricht</label>
                <input
                  class={styles.input}
                  type="text"
                  value={welcome()}
                  onInput={(e) => setWelcome(e.currentTarget.value)}
                  disabled={busy()}
                  placeholder="Willkommen auf dem Server!"
                />
              </div>
              <div class={styles.fieldRow}>
                <label class={styles.label}>Max. Clients</label>
                <input
                  class={styles.input}
                  type="number"
                  min="1"
                  value={maxClients()}
                  onInput={(e) => setMaxClients(e.currentTarget.value)}
                  disabled={busy()}
                />
              </div>
              <div class={styles.btnRow}>
                <button
                  type="submit"
                  class={styles.btnPrimary}
                  disabled={busy()}
                >
                  {busy() ? "Wird gespeichert..." : "Einstellungen speichern"}
                </button>
                {error() && <span class={styles.errorText}>{error()}</span>}
                {success() && <span class={styles.successText}>{success()}</span>}
              </div>
            </div>
          </div>
        </form>
      </Show>
    </div>
  );
}

// =======================================================================
// Audit-Log Tab
// =======================================================================

function AuditTab() {
  const PAGE_SIZE = 50;
  const [logs, setLogs] = createSignal<AuditLogEntry[]>([]);
  const [error, setError] = createSignal("");
  const [offset, setOffset] = createSignal(0);
  const [filter, setFilter] = createSignal("");
  const [hasMore, setHasMore] = createSignal(true);

  const load = async () => {
    try {
      const entries = await adminGetLogs(
        PAGE_SIZE,
        offset(),
        filter() || undefined
      );
      setLogs(entries);
      setHasMore(entries.length >= PAGE_SIZE);
      setError("");
    } catch (e) {
      setError(String(e));
    }
  };

  createEffect(() => {
    // Reagiert auf offset() und filter()
    void offset();
    void filter();
    load();
  });

  const prevPage = () => {
    const newOffset = offset() - PAGE_SIZE;
    setOffset(Math.max(0, newOffset));
  };

  const nextPage = () => {
    setOffset(offset() + PAGE_SIZE);
  };

  const handleFilterChange = (value: string) => {
    setOffset(0);
    setFilter(value);
  };

  return (
    <div>
      <Show when={error()}>
        <div class={styles.errorText}>{error()}</div>
      </Show>

      <div class={styles.toolbar}>
        <select
          class={styles.filterSelect}
          value={filter()}
          onChange={(e) => handleFilterChange(e.currentTarget.value)}
        >
          <option value="">Alle Aktionen</option>
          <option value="server.start">Server Start</option>
          <option value="client.gekickt">Client Kick</option>
          <option value="client.gebannt">Client Ban</option>
          <option value="kanal.erstellt">Channel erstellt</option>
          <option value="kanal.bearbeitet">Channel bearbeitet</option>
          <option value="kanal.geloescht">Channel geloescht</option>
          <option value="berechtigung.gesetzt">Berechtigung gesetzt</option>
          <option value="datei.geloescht">Datei geloescht</option>
        </select>
        <div class={styles.toolbarRight}>
          <button class={styles.actionBtn} onClick={load}>
            Aktualisieren
          </button>
        </div>
      </div>

      <Show
        when={logs().length > 0}
        fallback={<div class={styles.emptyState}>Keine Log-Eintraege</div>}
      >
        <table class={styles.table}>
          <thead>
            <tr>
              <th>Zeitpunkt</th>
              <th>Aktion</th>
              <th>Ausfuehrender</th>
              <th>Ziel</th>
              <th>Details</th>
            </tr>
          </thead>
          <tbody>
            <For each={logs()}>
              {(entry) => (
                <tr>
                  <td class={styles.mono}>
                    {formatTimestamp(entry.zeitstempel)}
                  </td>
                  <td>
                    <span class={styles.badge}>{entry.aktion}</span>
                  </td>
                  <td class={styles.mono}>{entry.aktor_id ?? "System"}</td>
                  <td class={styles.mono}>
                    {entry.ziel_typ
                      ? `${entry.ziel_typ}:${entry.ziel_id ?? ""}`
                      : "-"}
                  </td>
                  <td class={styles.mono}>
                    {entry.details &&
                    Object.keys(entry.details).length > 0
                      ? JSON.stringify(entry.details)
                      : "-"}
                  </td>
                </tr>
              )}
            </For>
          </tbody>
        </table>

        <div class={styles.pagination}>
          <button
            class={styles.paginationBtn}
            onClick={prevPage}
            disabled={offset() === 0}
          >
            Zurueck
          </button>
          <span class={styles.paginationInfo}>
            {offset() + 1} - {offset() + logs().length}
          </span>
          <button
            class={styles.paginationBtn}
            onClick={nextPage}
            disabled={!hasMore()}
          >
            Weiter
          </button>
        </div>
      </Show>
    </div>
  );
}

// =======================================================================
// Einladungen-Tab (Placeholder - REST-Endpunkte noch nicht vorhanden)
// =======================================================================

function InvitesTab() {
  return (
    <div>
      <div class={styles.emptyState}>
        Einladungs-Verwaltung wird in einer zukuenftigen Version verfuegbar sein.
        <br />
        Die Server-seitige Logik (InviteService) existiert bereits,
        <br />
        REST-Endpunkte fuer /v1/invites muessen noch implementiert werden.
      </div>
    </div>
  );
}

// =======================================================================
// Haupt-Komponente: AdminPanel
// =======================================================================

export default function AdminPanel() {
  const [activeTab, setActiveTab] = createSignal<TabId>("overview");

  const tabContent = (): JSX.Element => {
    switch (activeTab()) {
      case "overview":
        return <OverviewTab />;
      case "users":
        return <UsersTab />;
      case "bans":
        return <BansTab />;
      case "settings":
        return <SettingsTab />;
      case "audit":
        return <AuditTab />;
      case "invites":
        return <InvitesTab />;
      default:
        return <div />;
    }
  };

  return (
    <div class={styles.page}>
      {/* Breadcrumb */}
      <nav class={styles.breadcrumb}>
        <A href="/" class={styles.breadcrumbLink}>
          Server-Browser
        </A>
        <span class={styles.breadcrumbSep}>|</span>
        <span>Server-Administration</span>
      </nav>

      {/* Tab-Leiste */}
      <div class={styles.tabBar}>
        <For each={TABS}>
          {(tab) => (
            <button
              class={`${styles.tab} ${
                activeTab() === tab.id ? styles.tabActive : ""
              }`}
              onClick={() => setActiveTab(tab.id)}
            >
              {tab.label}
            </button>
          )}
        </For>
      </div>

      {/* Tab-Inhalt */}
      <div class={styles.tabContent}>{tabContent()}</div>
    </div>
  );
}
