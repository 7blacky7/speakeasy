import styles from "./PresetCard.module.css";

export interface PresetInfo {
  id: "speech" | "balanced" | "music" | "low_bandwidth";
  label: string;
  description: string;
  bitrate: string;
  sampleRate: string;
  channels: string;
  extras: string;
}

interface PresetCardProps {
  preset: PresetInfo;
  active: boolean;
  onSelect: () => void;
}

export default function PresetCard(props: PresetCardProps) {
  return (
    <button
      class={`${styles.card} ${props.active ? styles.active : ""}`}
      onClick={props.onSelect}
      aria-pressed={props.active}
      aria-label={`Preset: ${props.preset.label}`}
    >
      <div class={styles.header}>
        <span class={styles.name}>{props.preset.label}</span>
        {props.active && <span class={styles.badge}>Aktiv</span>}
      </div>
      <p class={styles.desc}>{props.preset.description}</p>
      <div class={styles.meta}>
        <span class={styles.metaItem}>{props.preset.bitrate}</span>
        <span class={styles.metaItem}>{props.preset.sampleRate}</span>
        <span class={styles.metaItem}>{props.preset.channels}</span>
        {props.preset.extras && (
          <span class={styles.metaItem}>{props.preset.extras}</span>
        )}
      </div>
    </button>
  );
}
