import { AudioStats } from "../../bridge";
import styles from "./LiveMonitor.module.css";

interface LiveMonitorProps {
  stats: AudioStats;
}

function LevelBar(props: { value: number; label: string; isClipping?: boolean }) {
  const pct = () => Math.min(100, Math.max(0, props.value * 100));

  const colorClass = () => {
    if (props.isClipping) return styles.levelClip;
    if (pct() > 85) return styles.levelHigh;
    if (pct() > 60) return styles.levelMid;
    return styles.levelNormal;
  };

  return (
    <div class={styles.levelRow}>
      <span class={styles.levelLabel}>{props.label}</span>
      <div class={styles.levelTrack} role="progressbar" aria-valuenow={Math.round(pct())} aria-valuemin={0} aria-valuemax={100} aria-label={props.label}>
        <div
          class={`${styles.levelFill} ${colorClass()}`}
          style={{ width: `${pct()}%` }}
        />
      </div>
      <span class={styles.levelPct}>{Math.round(pct())}%</span>
    </div>
  );
}

export default function LiveMonitor(props: LiveMonitorProps) {
  return (
    <div class={styles.monitor}>
      <div class={styles.levels}>
        <LevelBar
          value={props.stats.inputLevel}
          label="Eingang"
          isClipping={props.stats.isClipping}
        />
        <LevelBar
          value={props.stats.processedLevel}
          label="Nach DSP"
        />
      </div>

      <div class={styles.indicators}>
        <div class={`${styles.clipIndicator} ${props.stats.isClipping ? styles.clipping : ""}`}
          title="Clipping-Indikator"
          aria-label={props.stats.isClipping ? "Clipping aktiv" : "Kein Clipping"}
        >
          CLIP
        </div>
        <span class={styles.noiseFloor} title="Gerauschpegel">
          NF: {props.stats.noiseFloor.toFixed(1)} dBFS
        </span>
      </div>

      <div class={styles.stats}>
        <span class={styles.statBadge} title="Latenz">
          {props.stats.latency.total} ms
        </span>
        <span class={styles.statBadge} title="Paketverlust">
          PL {props.stats.packetLoss.toFixed(1)}%
        </span>
        <span class={styles.statBadge} title="Round-Trip-Time">
          RTT {props.stats.rtt} ms
        </span>
        <span class={styles.statBadge} title="Bitrate">
          {props.stats.bitrate} kbps
        </span>
      </div>
    </div>
  );
}
