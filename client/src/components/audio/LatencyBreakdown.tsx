import { For } from "solid-js";
import { LatencyBreakdown as LatencyData } from "../../bridge";
import styles from "./LatencyBreakdown.module.css";

interface LatencyBreakdownProps {
  data: LatencyData;
}

export default function LatencyBreakdown(props: LatencyBreakdownProps) {
  const rows = () => [
    { label: "Geratelatenz", value: props.data.device },
    { label: "Opus Kodierung", value: props.data.encoding },
    { label: "Jitter Buffer", value: props.data.jitter },
    { label: "Netzwerk RTT", value: props.data.network },
  ];

  return (
    <div class={styles.container}>
      <For each={rows()}>
        {(row) => (
          <div class={styles.row}>
            <span class={styles.label}>{row.label}</span>
            <span class={styles.value}>{row.value} ms</span>
          </div>
        )}
      </For>
      <div class={`${styles.row} ${styles.total}`}>
        <span class={styles.label}>Gesamt</span>
        <span class={styles.value}>{props.data.total} ms</span>
      </div>
    </div>
  );
}
