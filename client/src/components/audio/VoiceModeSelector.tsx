import { Show } from "solid-js";
import styles from "./VoiceModeSelector.module.css";

type VoiceMode = "ptt_hold" | "ptt_toggle" | "vad";

interface VoiceModeSelectorProps {
  mode: VoiceMode;
  pttKey: string | null;
  vadSensitivity: number;
  onModeChange: (mode: VoiceMode) => void;
  onPttKeyChange: () => void;
  onVadSensitivityChange: (value: number) => void;
}

const MODE_OPTIONS: { value: VoiceMode; label: string }[] = [
  { value: "ptt_hold", label: "Push-to-Talk (Halten)" },
  { value: "ptt_toggle", label: "Push-to-Talk (Umschalten)" },
  { value: "vad", label: "Sprachaktivierung (VAD)" },
];

export default function VoiceModeSelector(props: VoiceModeSelectorProps) {
  return (
    <div class={styles.container}>
      <div class={styles.toggleGroup} role="group" aria-label="Sprach-Modus">
        {MODE_OPTIONS.map((opt) => (
          <button
            class={`${styles.modeBtn} ${props.mode === opt.value ? styles.active : ""}`}
            onClick={() => props.onModeChange(opt.value)}
            aria-pressed={props.mode === opt.value}
          >
            {opt.label}
          </button>
        ))}
      </div>

      <Show when={props.mode === "ptt_hold" || props.mode === "ptt_toggle"}>
        <div class={styles.pttRow}>
          <span class={styles.pttLabel}>Taste:</span>
          <span class={styles.pttKey}>
            {props.pttKey ?? "Nicht festgelegt"}
          </span>
          <button class={styles.changeBtn} onClick={props.onPttKeyChange}>
            Andern
          </button>
        </div>
      </Show>

      <Show when={props.mode === "vad"}>
        <div class={styles.vadRow}>
          <label class={styles.vadLabel}>
            Empfindlichkeit
            <span class={styles.vadValue}>
              {Math.round(props.vadSensitivity * 100)}%
            </span>
          </label>
          <input
            type="range"
            class={styles.slider}
            min="0"
            max="1"
            step="0.01"
            value={props.vadSensitivity}
            onInput={(e) =>
              props.onVadSensitivityChange(
                parseFloat(e.currentTarget.value)
              )
            }
            aria-label="VAD Empfindlichkeit"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(props.vadSensitivity * 100)}
          />
        </div>
      </Show>
    </div>
  );
}
