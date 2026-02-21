import { For } from "solid-js";
import PresetCard, { PresetInfo } from "./PresetCard";
import styles from "./PresetGrid.module.css";

const PRESETS: PresetInfo[] = [
  {
    id: "speech",
    label: "Sprache",
    description: "Optimiert fur Sprachkommunikation mit niedrigem Datenverbrauch.",
    bitrate: "32 kbps",
    sampleRate: "16 kHz",
    channels: "Mono",
    extras: "FEC + DTX",
  },
  {
    id: "balanced",
    label: "Ausgewogen",
    description: "Gutes Gleichgewicht zwischen Qualitat und Datenverbrauch.",
    bitrate: "64 kbps",
    sampleRate: "48 kHz",
    channels: "Mono",
    extras: "",
  },
  {
    id: "music",
    label: "Musik",
    description: "Hochqualitative Ubertragung fur Musikstreaming.",
    bitrate: "192 kbps",
    sampleRate: "48 kHz",
    channels: "Stereo",
    extras: "CBR",
  },
  {
    id: "low_bandwidth",
    label: "Sparsam",
    description: "Minimaler Datenverbrauch fur schlechte Verbindungen.",
    bitrate: "12 kbps",
    sampleRate: "8 kHz",
    channels: "Mono",
    extras: "FEC + DTX",
  },
];

interface PresetGridProps {
  activePreset: string;
  onSelect: (id: "speech" | "balanced" | "music" | "low_bandwidth") => void;
}

export default function PresetGrid(props: PresetGridProps) {
  return (
    <div class={styles.grid}>
      <For each={PRESETS}>
        {(preset) => (
          <PresetCard
            preset={preset}
            active={props.activePreset === preset.id}
            onSelect={() => props.onSelect(preset.id)}
          />
        )}
      </For>
    </div>
  );
}
