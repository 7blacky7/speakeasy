import { createSignal, createEffect, onCleanup, onMount, Show } from "solid-js";
import { useParams, useNavigate } from "@solidjs/router";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { getServerInfo, joinChannel, disconnect, getCurrentUsername, type ChannelInfo } from "../bridge";
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

  // MenuBar-Handler
  const handleConnect = () => {
    setShowConnectDialog(true);
  };

  const handleConnected = async () => {
    setShowConnectDialog(false);
    setConnected(true);
    try {
      const name = await getCurrentUsername();
      setCurrentUsername(name);
    } catch {
      // ignorieren
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
    setServerName("");
    setChannels([]);
    setRawChannels([]);
    setSelectedChannel(null);
    setCurrentChannelId(null);
    setCurrentUsername(null);
    setError(null);
    setLoading(false);
    navigate("/");
  };

  // Tab-Daten
  const tabs = (): ServerTab[] => [
    { id: params.id, name: serverName() || "Server", active: true },
  ];

  const handleTabSelect = (_tabId: string) => {
    // Aktuell nur ein Tab
  };

  const handleTabClose = (_tabId: string) => {
    handleDisconnect();
  };

  const handleNewTab = () => {
    setShowConnectDialog(true);
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
      <Show when={connected()}>
        <TabBar
          tabs={tabs()}
          onTabSelect={handleTabSelect}
          onTabClose={handleTabClose}
          onNewTab={handleNewTab}
        />
      </Show>

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
