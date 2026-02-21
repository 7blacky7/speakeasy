import { Show } from "solid-js";
import type { ChannelNode } from "./ChannelTree";
import styles from "./ChannelInfo.module.css";

interface ChannelInfoProps {
  channel: ChannelNode | null;
}

export default function ChannelInfo(props: ChannelInfoProps) {
  return (
    <div class={styles.panel}>
      <Show
        when={props.channel}
        fallback={
          <div class={styles.placeholder}>
            <span class={styles.placeholderText}>Kanal auswaehlen um Details zu sehen</span>
          </div>
        }
      >
        {(ch) => (
          <div class={styles.content}>
            <div class={styles.header}>
              <span class={styles.channelName}>{ch().name}</span>
            </div>

            <Show when={ch().description}>
              <div class={styles.section}>
                <div class={styles.sectionTitle}>Beschreibung</div>
                <div class={styles.sectionContent}>{ch().description}</div>
              </div>
            </Show>

            <div class={styles.section}>
              <div class={styles.sectionTitle}>Details</div>
              <div class={styles.detailGrid}>
                <div class={styles.detailRow}>
                  <span class={styles.detailLabel}>Typ</span>
                  <span class={styles.detailValue}>{channelTypeLabel(ch().channel_type)}</span>
                </div>
                <div class={styles.detailRow}>
                  <span class={styles.detailLabel}>Clients</span>
                  <span class={styles.detailValue}>
                    {ch().clients.length}
                    {ch().max_clients > 0 ? `/${ch().max_clients}` : "/unbegrenzt"}
                  </span>
                </div>
                <Show when={ch().has_password}>
                  <div class={styles.detailRow}>
                    <span class={styles.detailLabel}>Passwort</span>
                    <span class={styles.detailValue}>Ja</span>
                  </div>
                </Show>
                <div class={styles.detailRow}>
                  <span class={styles.detailLabel}>Subchannels</span>
                  <span class={styles.detailValue}>{ch().children.length}</span>
                </div>
              </div>
            </div>
          </div>
        )}
      </Show>
    </div>
  );
}

function channelTypeLabel(type: string): string {
  switch (type) {
    case "permanent": return "Permanent";
    case "semi_permanent": return "Semi-Permanent";
    case "temporary": return "Temporaer";
    default: return type;
  }
}
