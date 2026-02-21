import { createResource, createSignal, For, Show } from "solid-js";
import { useParams } from "@solidjs/router";
import { getServerInfo, joinChannel, type ChannelInfo } from "../bridge";
import { ChatPanel } from "../components/chat/ChatPanel";
import styles from "./ServerView.module.css";

export default function ServerView() {
  const params = useParams<{ id: string }>();
  const [serverInfo] = createResource(() => params.id, getServerInfo);
  const [activeChannel, setActiveChannel] = createSignal<ChannelInfo | null>(null);

  const handleChannelClick = async (channel: ChannelInfo) => {
    setActiveChannel(channel);
    try {
      await joinChannel(channel.id);
    } catch (e) {
      console.error("Kanal beitreten fehlgeschlagen:", e);
    }
  };

  return (
    <div class={styles.page}>
      <Show
        when={!serverInfo.loading}
        fallback={<div class={styles.loading}>Lade Serverinfo...</div>}
      >
        <Show
          when={serverInfo()}
          fallback={
            <div class={styles.placeholder}>
              <p>Server nicht erreichbar.</p>
            </div>
          }
        >
          {(info) => (
            <div class={styles.serverContent}>
              {/* Channel-Liste links */}
              <div class={styles.channelSidebar}>
                <div class={styles.serverHeader}>
                  <div class={styles.serverName}>{info().name}</div>
                  <div class={styles.serverMeta}>
                    <span class={styles.metaBadge}>
                      {info().online_clients}/{info().max_clients}
                    </span>
                    <span class={styles.metaBadge}>v{info().version}</span>
                  </div>
                </div>

                <div class={styles.channelListScroll}>
                  <div class={styles.sectionLabel}>Kanaele</div>
                  <For each={info().channels}>
                    {(channel) => (
                      <>
                        <button
                          class={`${styles.channelItem} ${activeChannel()?.id === channel.id ? styles.active : ""}`}
                          onClick={() => handleChannelClick(channel)}
                        >
                          <span class={styles.channelIcon}>
                            {channel.description?.includes("[text]") ? "#" : "🔊"}
                          </span>
                          <span class={styles.channelName}>{channel.name}</span>
                          <Show when={channel.clients.length > 0}>
                            <span class={styles.userCount}>
                              {channel.clients.length}
                            </span>
                          </Show>
                        </button>

                        {/* Mitglieder im Voice-Kanal unterhalb anzeigen */}
                        <Show when={channel.clients.length > 0}>
                          <div class={styles.memberList}>
                            <For each={channel.clients}>
                              {(client) => (
                                <div class={styles.memberEntry}>
                                  <div class={styles.memberAvatar}>
                                    {client.username[0].toUpperCase()}
                                  </div>
                                  <span class={styles.memberName}>
                                    {client.username}
                                    {client.is_self ? " (Du)" : ""}
                                  </span>
                                  <div class={styles.memberIcons}>
                                    {client.is_muted && (
                                      <span title="Stummgeschaltet">🔇</span>
                                    )}
                                    {client.is_deafened && (
                                      <span title="Taub">🔕</span>
                                    )}
                                  </div>
                                </div>
                              )}
                            </For>
                          </div>
                        </Show>
                      </>
                    )}
                  </For>
                </div>
              </div>

              {/* Chat-Panel rechts */}
              <ChatPanel channel={activeChannel()} />
            </div>
          )}
        </Show>
      </Show>
    </div>
  );
}
