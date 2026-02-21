import styles from "./LatencyDisplay.module.css";

interface LatencyDisplayProps {
  latencyMs: number;
}

export default function LatencyDisplay(props: LatencyDisplayProps) {
  const colorClass = () => {
    if (props.latencyMs < 30) return styles.good;
    if (props.latencyMs < 60) return styles.warn;
    return styles.bad;
  };

  return (
    <div class={`${styles.display} ${colorClass()}`} aria-label={`Latenz: ${props.latencyMs} Millisekunden`}>
      <span class={styles.dot} />
      <span class={styles.label}>Latenz:</span>
      <span class={styles.value}>{props.latencyMs} ms</span>
    </div>
  );
}
