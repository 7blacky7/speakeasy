import { createSignal, For, Show } from "solid-js";
import { A, useNavigate } from "@solidjs/router";
import styles from "./Sidebar.module.css";

interface ServerEntry {
  id: string;
  name: string;
  initials: string;
  connected: boolean;
}

interface Channel {
  id: string;
  name: string;
  type: "voice" | "text";
  userCount?: number;
  children?: Channel[];
}

const DEMO_SERVERS: ServerEntry[] = [
  { id: "1", name: "Mein Server", initials: "MS", connected: true },
  { id: "2", name: "Gaming Crew", initials: "GC", connected: false },
];

const DEMO_CHANNELS: Channel[] = [
  {
    id: "1",
    name: "Allgemein",
    type: "voice",
    userCount: 3,
    children: [],
  },
  {
    id: "2",
    name: "Gaming",
    type: "voice",
    userCount: 0,
    children: [],
  },
  {
    id: "3",
    name: "AFK",
    type: "voice",
    userCount: 1,
    children: [],
  },
];

export default function Sidebar() {
  const [activeServer, setActiveServer] = createSignal<string | null>("1");
  const navigate = useNavigate();

  function selectServer(server: ServerEntry) {
    setActiveServer(server.id);
    navigate(`/server/${server.id}`);
  }

  return (
    <aside class={`${styles.sidebar} no-select`}>
      {/* Server-Liste */}
      <div class={styles.serverList}>
        <A href="/" class={styles.homeBtn} title="Server-Browser">
          <span>⌂</span>
        </A>
        <div class={styles.separator} />
        <For each={DEMO_SERVERS}>
          {(server) => (
            <button
              class={`${styles.serverIcon} ${activeServer() === server.id ? styles.active : ""} ${server.connected ? styles.connected : ""}`}
              onClick={() => selectServer(server)}
              title={server.name}
            >
              <span class={styles.serverInitials}>{server.initials}</span>
              {server.connected && <span class={styles.connectedDot} />}
            </button>
          )}
        </For>
        <button class={styles.addServerBtn} title="Server hinzufügen">
          <span>+</span>
        </button>
      </div>

      {/* Channel-Liste (nur bei aktivem Server) */}
      <Show when={activeServer()}>
        <div class={styles.channelList}>
          <div class={styles.channelListHeader}>
            <span class={styles.serverName}>
              {DEMO_SERVERS.find((s) => s.id === activeServer())?.name}
            </span>
          </div>
          <div class={styles.channels}>
            <div class={styles.channelSection}>
              <span class={styles.sectionLabel}>Sprachkanäle</span>
            </div>
            <For each={DEMO_CHANNELS}>
              {(channel) => (
                <button
                  class={styles.channelItem}
                  title={channel.name}
                >
                  <span class={styles.channelIcon}>
                    {channel.type === "voice" ? "🔊" : "#"}
                  </span>
                  <span class={styles.channelName}>{channel.name}</span>
                  <Show when={channel.userCount && channel.userCount > 0}>
                    <span class={styles.userCount}>{channel.userCount}</span>
                  </Show>
                </button>
              )}
            </For>
          </div>
        </div>
      </Show>
    </aside>
  );
}
