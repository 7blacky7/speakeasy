import { createSignal, createEffect, onCleanup, onMount, Show } from "solid-js";
import { useParams, useNavigate } from "@solidjs/router";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getServerInfo, joinChannel, disconnect, connectToServer, getCurrentUsername, type ChannelInfo } from "../bridge";
import ChannelTree, { buildChannelTree, type ChannelNode } from "../components/server/ChannelTree";
import ChannelInfoPanel from "../components/server/ChannelInfo";
import ServerInfoPanel from "../components/server/ServerInfoPanel";
import MenuBar from "../components/ui/MenuBar";
import TabBar, { type ServerTab } from "../components/ui/TabBar";
import { ChatPanel } from "../components/chat/ChatPanel";
import ChannelCreateDialog from "../components/server/ChannelCreateDialog";
import ChannelEditDialog from "../components/server/ChannelEditDialog";
import ChannelDeleteDialog from "../components/server/ChannelDeleteDialog";
import ConnectDialog from "../components/server/ConnectDialog";
import {
  getTabs, getActiveTabId, getActiveTab, setActiveTab,
  addTab, removeTab, updateTab, reorderTabs,
} from "../stores/connectionStore";
import styles from "./ServerView.module.css";

type DialogState =
  | { type: "none" }
  | { type: "create"; parentId: string | null }
  | { type: "edit"; channelId: string }
  | { type: "delete"; channelId: string; channelName: string };

type InfoPanelMode = "server" | "channel";

export default function ServerView() {
  const params = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [serverName, setServerName] = createSignal("");
  const [serverDescription, setServerDescription] = createSignal("");
  const [serverVersion, setServerVersion] = createSignal("");
  const [onlineClients, setOnlineClients] = createSignal(0);
  const [maxClients, setMaxClients] = createSignal(0);
  const [uptimeSecs, setUptimeSecs] = createSignal(0);
  const [channels, setChannels] = createSignal<ChannelNode[]>([]);
  const [rawChannels, setRawChannels] = createSignal<ChannelInfo[]>([]);
  const [selectedChannel, setSelectedChannel] = createSignal<ChannelNode | null>(null);
  const [currentChannelId, setCurrentChannelId] = createSignal<string | null>(null);
  const [chatVisible, setChatVisible] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(false);
  const [dialog, setDialog] = createSignal<DialogState>({ type: "none" });
  const [infoPanelMode, setInfoPanelMode] = createSignal<InfoPanelMode>("server");
  const [currentUsername, setCurrentUsername] = createSignal<string | null>(null);
  const [connected, setConnected] = createSignal(false);
  const [showConnectDialog, setShowConnectDialog] = createSignal(false);

  onMount(async () => {
    try {
      const name = await getCurrentUsername();
      if (name) {
        setCurrentUsername(name);
        // Wir haben einen Username, also versuchen wir Server-Info zu laden
        setConnected(true);
        // Store synchronisieren mit bestehender Verbindung
        const addr = localStorage.getItem("speakeasy_last_address") || "";
        const port = Number(localStorage.getItem("speakeasy_last_port")) || 9001;
        const user = localStorage.getItem("speakeasy_last_username") || name;
        updateTab(getActiveTabId(), {
          connected: true,
          name: addr || "Server",
          address: addr,
          port,
          username: user,
        });
      }
    } catch {
      // kein Username verfuegbar - nicht verbunden
    }
  });

  // Server-Info polling (alle 4 Sekunden)
  let pollTimer: number | undefined;

  const fetchServerInfo = async () => {
    try {
      const info = await getServerInfo();
      setServerName(info.name);
      setServerDescription(info.description);
      setServerVersion(info.version);
      setOnlineClients(info.online_clients);
      setMaxClients(info.max_clients);
      setUptimeSecs(info.uptime_secs);
      setRawChannels(info.channels);
      setChannels(buildChannelTree(info.channels));
      setError(null);
      setLoading(false);
      // Tab-Name mit dem echten Servernamen aktualisieren
      if (info.name) {
        updateTab(getActiveTabId(), { name: info.name });
      }
    } catch {
      setError("Server nicht erreichbar");
      setLoading(false);
    }
  };

  const startPolling = () => {
    if (pollTimer) clearInterval(pollTimer);
    setLoading(true);
    fetchServerInfo();
    pollTimer = window.setInterval(fetchServerInfo, 4000);
  };

  createEffect(() => {
    if (connected()) {
      // Bei Verbindung oder Server-Wechsel neu laden
      void params.id;
      startPolling();
    }
  });

  onCleanup(() => {
    if (pollTimer) clearInterval(pollTimer);
  });

  const handleChannelJoin = async (channelId: string) => {
    try {
      await joinChannel(channelId);
      setCurrentChannelId(channelId);
      // Sofort Server-Info aktualisieren damit der Wechsel instant sichtbar ist
      fetchServerInfo();
    } catch (e) {
      console.error("Kanal beitreten fehlgeschlagen:", e);
    }
  };

  const handleChannelSelect = (channel: ChannelNode) => {
    setSelectedChannel(channel);
    setInfoPanelMode("channel");
  };

  const handleServerClick = () => {
    setSelectedChannel(null);
    setInfoPanelMode("server");
  };

  // --- Dialog-Handler ---

  const handleChannelCreate = () => {
    setDialog({ type: "create", parentId: null });
  };

  const handleChannelEdit = (channelId: string) => {
    setDialog({ type: "edit", channelId });
  };

  const handleChannelDelete = (channelId: string) => {
    const ch = rawChannels().find((c) => c.id === channelId);
    if (!ch) return;
    setDialog({ type: "delete", channelId, channelName: ch.name });
  };

  const handleSubchannelCreate = (parentId: string) => {
    setDialog({ type: "create", parentId });
  };

  const closeDialog = () => {
    setDialog({ type: "none" });
  };

  const handleDialogDone = () => {
    fetchServerInfo();
  };

  // Aktuell bearbeiteten Channel finden
  const editChannel = () => {
    const d = dialog();
    if (d.type !== "edit") return null;
    return rawChannels().find((c) => c.id === d.channelId) ?? null;
  };

  const toggleChat = () => {
    setChatVisible((v) => !v);
  };

  // Keyboard-Shortcut: Strg+Enter fuer Chat ein/aus
  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.ctrlKey && e.key === "Enter") {
      toggleChat();
    }
  };

  createEffect(() => {
    document.addEventListener("keydown", handleKeyDown);
    onCleanup(() => document.removeEventListener("keydown", handleKeyDown));
  });

  // Aktiven Channel als ChannelInfo fuer ChatPanel finden
  const activeChatChannel = () => {
    const id = currentChannelId();
    if (!id) return null;
    return rawChannels().find((c) => c.id === id) ?? null;
  };

  // View-State zuruecksetzen (ohne Navigation)
  const resetViewState = () => {
    setServerName("");
    setChannels([]);
    setRawChannels([]);
    setSelectedChannel(null);
    setCurrentChannelId(null);
    setCurrentUsername(null);
    setError(null);
    setLoading(false);
  };

  // MenuBar-Handler
  const handleConnect = () => {
    setShowConnectDialog(true);
  };

  const handleConnected = async (details?: { address: string; port: number; username: string; password?: string }) => {
    setShowConnectDialog(false);
    setConnected(true);
    try {
      const name = await getCurrentUsername();
      setCurrentUsername(name);
    } catch {
      // ignorieren
    }
    // Store mit Verbindungsdetails aktualisieren
    if (details) {
      updateTab(getActiveTabId(), {
        connected: true,
        name: details.address === "localhost" ? `${details.address}` : details.address,
        address: details.address,
        port: details.port,
        username: details.username,
        password: details.password,
      });
    } else {
      updateTab(getActiveTabId(), { connected: true });
    }
    if (!params.id) {
      navigate("/server/1");
    }
  };

  const handleDisconnect = async () => {
    try {
      await disconnect();
    } catch (e) {
      console.error("Trennen fehlgeschlagen:", e);
    }
    if (pollTimer) clearInterval(pollTimer);
    setConnected(false);
    resetViewState();
    // Store aktualisieren: Tab als nicht verbunden markieren
    updateTab(getActiveTabId(), { connected: false, name: "Nicht verbunden" });
    navigate("/");
  };

  // Tab-Daten aus dem Connection-Store
  const tabs = (): ServerTab[] =>
    getTabs().map(t => ({
      id: t.id,
      name: t.name,
      active: t.id === getActiveTabId(),
    }));

  const handleTabSelect = async (tabId: string) => {
    if (tabId === getActiveTabId()) return;

    // Aktuelle Verbindung trennen (Backend unterstuetzt nur 1 gleichzeitig)
    if (connected()) {
      try {
        await disconnect();
      } catch (e) {
        console.error("Trennen beim Tab-Wechsel fehlgeschlagen:", e);
      }
      if (pollTimer) clearInterval(pollTimer);
      resetViewState();
    }

    setActiveTab(tabId);

    // Zum neuen Tab verbinden, falls dieser Verbindungsdaten hat
    const tab = getActiveTab();
    if (tab && tab.connected && tab.address && tab.username) {
      try {
        await connectToServer({
          address: tab.address,
          port: tab.port,
          username: tab.username,
          password: tab.password,
        });
        setConnected(true);
        setCurrentUsername(tab.username);
        if (!params.id) navigate("/server/1");
      } catch (e) {
        console.error("Verbinden beim Tab-Wechsel fehlgeschlagen:", e);
        updateTab(tabId, { connected: false });
        setConnected(false);
      }
    } else {
      setConnected(false);
    }
  };

  const handleTabClose = async (tabId: string) => {
    const wasActive = tabId === getActiveTabId();

    // Wenn der geschlossene Tab gerade verbunden und aktiv ist: trennen
    if (wasActive && connected()) {
      try {
        await disconnect();
      } catch (e) {
        console.error("Trennen beim Tab-Schliessen fehlgeschlagen:", e);
      }
      if (pollTimer) clearInterval(pollTimer);
      resetViewState();
    }

    removeTab(tabId);

    // Wenn der aktive Tab geschlossen wurde, zum neuen aktiven Tab wechseln
    if (wasActive) {
      const newActive = getActiveTab();
      if (newActive && newActive.connected && newActive.address && newActive.username) {
        try {
          await connectToServer({
            address: newActive.address,
            port: newActive.port,
            username: newActive.username,
            password: newActive.password,
          });
          setConnected(true);
          setCurrentUsername(newActive.username);
        } catch {
          updateTab(newActive.id, { connected: false });
          setConnected(false);
        }
      } else {
        setConnected(false);
      }
    }
  };

  const handleNewTab = () => {
    const newId = crypto.randomUUID();
    addTab({
      id: newId,
      name: "Nicht verbunden",
      address: "",
      port: 9001,
      username: "",
      connected: false,
    });
    // Sofort zum neuen Tab wechseln (trennt aktive Verbindung)
    handleTabSelect(newId);
    setShowConnectDialog(true);
  };

  const handleTabsReorder = (reordered: ServerTab[]) => {
    reorderTabs(reordered.map(t => t.id));
  };

  const handleCloseOtherTabs = async (keepTabId: string) => {
    const toClose = getTabs().filter(t => t.id !== keepTabId);
    for (const t of toClose) {
      // Nur den aktiven Tab muss tatsaechlich getrennt werden
      if (t.id === getActiveTabId() && connected()) {
        try {
          await disconnect();
        } catch (e) {
          console.error("Trennen fehlgeschlagen:", e);
        }
        if (pollTimer) clearInterval(pollTimer);
        resetViewState();
      }
      removeTab(t.id);
    }
    // Sicherstellen dass der behaltene Tab aktiv ist
    if (getActiveTabId() !== keepTabId) {
      await handleTabSelect(keepTabId);
    }
  };

  // Admin-Navigation als separates Fenster
  const openAdminWindow = async () => {
    const label = "admin";
    const existing = await WebviewWindow.getByLabel(label);
    if (existing) {
      await existing.setFocus();
      return;
    }
    const baseUrl = window.location.origin;
    new WebviewWindow(label, {
      url: `${baseUrl}/admin`,
      title: "Server-Verwaltung",
      width: 950,
      height: 700,
      resizable: true,
      center: true,
      decorations: false,
    });
  };

  const handlePermissions = () => {
    openAdminWindow();
  };

  const handleAuditLog = () => {
    openAdminWindow();
  };

  return (
    <div class={styles.page}>
      {/* Menueleiste - IMMER sichtbar */}
      <MenuBar
        connected={connected() && !error()}
        serverName={serverName()}
        serverAddress={localStorage.getItem("speakeasy_last_address") || undefined}
        serverPort={Number(localStorage.getItem("speakeasy_last_port")) || undefined}
        username={currentUsername() || undefined}
        onConnect={handleConnect}
        onDisconnect={handleDisconnect}
      />

      {/* Tab-Leiste - IMMER sichtbar */}
      <TabBar
        tabs={tabs()}
        onTabSelect={handleTabSelect}
        onTabClose={handleTabClose}
        onNewTab={handleNewTab}
        onTabsReorder={handleTabsReorder}
        onCloseOtherTabs={handleCloseOtherTabs}
      />

      {/* Nicht verbunden: Hinweis */}
      <Show when={!connected()}>
        <div class={styles.disconnected}>
          <div class={styles.disconnectedText}>Nicht verbunden</div>
          <div class={styles.disconnectedHint}>
            Server &gt; Verbinden... um eine Verbindung herzustellen
          </div>
        </div>
      </Show>

      {/* Verbunden: Server-Interface */}
      <Show when={connected()}>
        <Show when={!loading()} fallback={<div class={styles.loading}>Lade Serverinfo...</div>}>
          <Show when={!error()} fallback={<div class={styles.error}>{error()}</div>}>
            {/* Hauptbereich: ChannelTree links + Info rechts */}
            <div class={styles.mainArea}>
              {/* Channel-Baum (links) */}
              <div class={styles.channelTreePanel}>
                <ChannelTree
                  channels={channels()}
                  currentChannelId={currentChannelId()}
                  currentUserId={null}
                  currentUsername={currentUsername()}
                  onChannelJoin={handleChannelJoin}
                  onChannelSelect={handleChannelSelect}
                  onChannelEdit={handleChannelEdit}
                  onChannelDelete={handleChannelDelete}
                  onSubchannelCreate={handleSubchannelCreate}
                  serverName={serverName()}
                  onlineClients={onlineClients()}
                  maxClients={maxClients()}
                  onServerClick={handleServerClick}
                  onServerEdit={() => openAdminWindow()}
                  onChannelCreate={handleChannelCreate}
                  onPermissions={handlePermissions}
                  onAuditLog={handleAuditLog}
                  serverSelected={infoPanelMode() === "server"}
                />
              </div>

              {/* Info-Panel (rechts) */}
              <Show
                when={infoPanelMode() === "channel" && selectedChannel()}
                fallback={
                  <ServerInfoPanel
                    name={serverName()}
                    description={serverDescription()}
                    version={serverVersion()}
                    onlineClients={onlineClients()}
                    maxClients={maxClients()}
                    uptimeSecs={uptimeSecs()}
                    isAdmin={true}
                    onServerUpdated={fetchServerInfo}
                  />
                }
              >
                <ChannelInfoPanel channel={selectedChannel()} />
              </Show>
            </div>

            {/* Chat-Panel (unten, einklappbar) */}
            <div class={styles.chatToggle}>
              <button class={styles.chatToggleBtn} onClick={toggleChat}>
                {chatVisible() ? "Chat ausblenden" : "Chat einblenden"}
                <span class={styles.chatShortcut}>Strg+Enter</span>
              </button>
            </div>

            <Show when={chatVisible()}>
              <div class={styles.chatArea}>
                <ChatPanel channel={activeChatChannel()} />
              </div>
            </Show>
          </Show>
        </Show>
      </Show>

      {/* ConnectDialog (Modal) */}
      <Show when={showConnectDialog()}>
        <ConnectDialog
          onClose={() => setShowConnectDialog(false)}
          onConnected={handleConnected}
        />
      </Show>

      {/* Dialoge */}
      <Show when={dialog().type === "create"}>
        <ChannelCreateDialog
          channels={rawChannels()}
          defaultParentId={(dialog() as { type: "create"; parentId: string | null }).parentId}
          onClose={closeDialog}
          onCreated={handleDialogDone}
        />
      </Show>

      <Show when={dialog().type === "edit" && editChannel() !== null}>
        <ChannelEditDialog
          channel={editChannel()!}
          channels={rawChannels()}
          onClose={closeDialog}
          onEdited={handleDialogDone}
        />
      </Show>

      <Show when={dialog().type === "delete"}>
        {(() => {
          const d = dialog() as { type: "delete"; channelId: string; channelName: string };
          return (
            <ChannelDeleteDialog
              channelId={d.channelId}
              channelName={d.channelName}
              onClose={closeDialog}
              onDeleted={handleDialogDone}
            />
          );
        })()}
      </Show>
    </div>
  );
}
