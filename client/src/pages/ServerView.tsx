import { createSignal, createEffect, onCleanup, Show } from "solid-js";
import { useParams } from "@solidjs/router";
import { getServerInfo, joinChannel, type ChannelInfo } from "../bridge";
import ChannelTree, { buildChannelTree, type ChannelNode } from "../components/server/ChannelTree";
import ChannelInfoPanel from "../components/server/ChannelInfo";
import { ChatPanel } from "../components/chat/ChatPanel";
import ChannelCreateDialog from "../components/server/ChannelCreateDialog";
import ChannelEditDialog from "../components/server/ChannelEditDialog";
import ChannelDeleteDialog from "../components/server/ChannelDeleteDialog";
import styles from "./ServerView.module.css";

type DialogState =
  | { type: "none" }
  | { type: "create"; parentId: string | null }
  | { type: "edit"; channelId: string }
  | { type: "delete"; channelId: string; channelName: string };

export default function ServerView() {
  const params = useParams<{ id: string }>();
  const [serverName, setServerName] = createSignal("");
  const [serverVersion, setServerVersion] = createSignal("");
  const [onlineClients, setOnlineClients] = createSignal(0);
  const [maxClients, setMaxClients] = createSignal(0);
  const [channels, setChannels] = createSignal<ChannelNode[]>([]);
  const [rawChannels, setRawChannels] = createSignal<ChannelInfo[]>([]);
  const [selectedChannel, setSelectedChannel] = createSignal<ChannelNode | null>(null);
  const [currentChannelId, setCurrentChannelId] = createSignal<string | null>(null);
  const [chatVisible, setChatVisible] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [loading, setLoading] = createSignal(true);
  const [dialog, setDialog] = createSignal<DialogState>({ type: "none" });

  // Server-Info polling (alle 4 Sekunden)
  let pollTimer: number | undefined;

  const fetchServerInfo = async () => {
    try {
      const info = await getServerInfo();
      setServerName(info.name);
      setServerVersion(info.version);
      setOnlineClients(info.online_clients);
      setMaxClients(info.max_clients);
      setRawChannels(info.channels);
      setChannels(buildChannelTree(info.channels));
      setError(null);
      setLoading(false);
    } catch (e) {
      setError("Server nicht erreichbar");
      setLoading(false);
    }
  };

  createEffect(() => {
    // Bei Server-Wechsel neu laden
    void params.id;
    setLoading(true);
    fetchServerInfo();

    pollTimer = window.setInterval(fetchServerInfo, 4000);
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

  return (
    <div class={styles.page}>
      <Show when={!loading()} fallback={<div class={styles.loading}>Lade Serverinfo...</div>}>
        <Show when={!error()} fallback={<div class={styles.error}>{error()}</div>}>
          {/* Server-Header */}
          <div class={styles.serverHeader}>
            <span class={styles.serverName}>{serverName()}</span>
            <div class={styles.serverMeta}>
              <span class={styles.metaBadge}>{onlineClients()}/{maxClients()} Clients</span>
              <span class={styles.metaBadge}>v{serverVersion()}</span>
              <button class={styles.createChannelBtn} onClick={handleChannelCreate}>
                + Channel
              </button>
            </div>
          </div>

          {/* Hauptbereich: ChannelTree links + ChannelInfo rechts */}
          <div class={styles.mainArea}>
            {/* Channel-Baum (links) */}
            <div class={styles.channelTreePanel}>
              <ChannelTree
                channels={channels()}
                currentChannelId={currentChannelId()}
                currentUserId={null}
                onChannelJoin={handleChannelJoin}
                onChannelSelect={handleChannelSelect}
                onChannelEdit={handleChannelEdit}
                onChannelDelete={handleChannelDelete}
                onSubchannelCreate={handleSubchannelCreate}
              />
            </div>

            {/* Channel-Info (rechts) */}
            <ChannelInfoPanel channel={selectedChannel()} />
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
