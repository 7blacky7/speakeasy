import { createResource, For, Show } from "solid-js";
import { useParams } from "@solidjs/router";
import { getServerInfo } from "../bridge";
import styles from "./ServerView.module.css";

export default function ServerView() {
  const params = useParams<{ id: string }>();
  const [serverInfo] = createResource(() => params.id, getServerInfo);

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
              <div class={styles.serverHeader}>
                <h2 class={styles.serverName}>{info().name}</h2>
                <p class={styles.serverDesc}>{info().description}</p>
                <div class={styles.serverMeta}>
                  <span class={styles.metaBadge}>
                    {info().online_clients} / {info().max_clients} Nutzer
                  </span>
                  <span class={styles.metaBadge}>v{info().version}</span>
                </div>
              </div>

              <div class={styles.channelArea}>
                <For each={info().channels}>
                  {(channel) => (
                    <div class={styles.channelBlock}>
                      <div class={styles.channelHeader}>
                        <span class={styles.channelIcon}>🔊</span>
                        <span class={styles.channelName}>{channel.name}</span>
                        <span class={styles.channelDesc}>
                          {channel.description}
                        </span>
                      </div>
                      <div class={styles.clientList}>
                        <For each={channel.clients}>
                          {(client) => (
                            <div
                              class={`${styles.clientEntry} ${client.is_self ? styles.self : ""}`}
                            >
                              <div class={styles.clientAvatar}>
                                {client.username[0].toUpperCase()}
                              </div>
                              <span class={styles.clientName}>
                                {client.username}
                                {client.is_self ? " (Du)" : ""}
                              </span>
                              <div class={styles.clientIcons}>
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
                    </div>
                  )}
                </For>
              </div>
            </div>
          )}
        </Show>
      </Show>
    </div>
  );
}
