import { For } from "solid-js";
import { CodecConfig } from "../../bridge";
import AudioSlider from "./AudioSlider";
import DspModule from "./DspModule";
import CustomSelect from "../ui/CustomSelect";
import styles from "./CodecSettings.module.css";

interface CodecSettingsProps {
  config: CodecConfig;
  onChange: <K extends keyof CodecConfig>(key: K, value: CodecConfig[K]) => void;
}

const SAMPLE_RATES = [8000, 12000, 16000, 24000, 48000];
const BUFFER_SIZES = [64, 128, 256, 512, 1024];
const FRAME_SIZES = [2.5, 5, 10, 20, 40, 60];

const APPLICATIONS: { value: CodecConfig["application"]; label: string; desc: string }[] = [
  { value: "voip", label: "VOIP", desc: "Sprachoptimiert, niedrige Latenz" },
  { value: "audio", label: "Audio", desc: "Allgemein, ausgewogene Qualitat" },
  { value: "low_delay", label: "Low Delay", desc: "Minimale algorithmische Latenz" },
];

export default function CodecSettings(props: CodecSettingsProps) {
  return (
    <div class={styles.container}>
      <div class={styles.row}>
        <label class={styles.label}>Sample Rate</label>
        <CustomSelect
          value={String(props.config.sampleRate)}
          options={SAMPLE_RATES.map((r) => ({ value: String(r), label: `${r} Hz` }))}
          onChange={(v) => props.onChange("sampleRate", parseInt(v))}
          ariaLabel="Sample Rate"
        />
      </div>

      <div class={styles.row}>
        <label class={styles.label}>Buffer Size</label>
        <CustomSelect
          value={String(props.config.bufferSize)}
          options={BUFFER_SIZES.map((s) => ({ value: String(s), label: `${s} Samples` }))}
          onChange={(v) => props.onChange("bufferSize", parseInt(v))}
          ariaLabel="Buffer Size"
        />
      </div>

      <AudioSlider
        label="Opus Bitrate"
        value={props.config.bitrate}
        min={6}
        max={510}
        step={2}
        unit=" kbps"
        onChange={(v) => props.onChange("bitrate", v)}
        showInput={true}
      />

      <div class={styles.row}>
        <label class={styles.label}>Frame Size</label>
        <CustomSelect
          value={String(props.config.frameSize)}
          options={FRAME_SIZES.map((s) => ({ value: String(s), label: `${s} ms` }))}
          onChange={(v) => props.onChange("frameSize", parseFloat(v))}
          ariaLabel="Frame Size"
        />
      </div>

      <div class={styles.radioGroup}>
        <span class={styles.label}>Opus Application</span>
        <div class={styles.radioOptions}>
          <For each={APPLICATIONS}>
            {(app) => (
              <label class={`${styles.radioOption} ${props.config.application === app.value ? styles.radioActive : ""}`}>
                <input
                  type="radio"
                  name="opus-application"
                  value={app.value}
                  checked={props.config.application === app.value}
                  onChange={() => props.onChange("application", app.value)}
                  class={styles.radioInput}
                />
                <div class={styles.radioContent}>
                  <span class={styles.radioLabel}>{app.label}</span>
                  <span class={styles.radioDesc}>{app.desc}</span>
                </div>
              </label>
            )}
          </For>
        </div>
      </div>

      <div class={styles.toggleRow}>
        <DspModule
          label="FEC (Forward Error Correction)"
          enabled={props.config.fec}
          onToggle={(v) => props.onChange("fec", v)}
          tooltip="Erhoht Robustheit bei Paketverlusten auf Kosten von Bandbreite"
        />
        <DspModule
          label="DTX (Discontinuous Transmission)"
          enabled={props.config.dtx}
          onToggle={(v) => props.onChange("dtx", v)}
          tooltip="Sendet keine Daten bei Stille - spart Bandbreite"
        />
      </div>

      <div class={styles.row}>
        <label class={styles.label}>Mikrofon Kanale</label>
        <div class={styles.channelToggle}>
          <button
            class={`${styles.channelBtn} ${props.config.channels === "mono" ? styles.channelActive : ""}`}
            onClick={() => props.onChange("channels", "mono")}
            aria-pressed={props.config.channels === "mono"}
          >
            Mono
          </button>
          <button
            class={`${styles.channelBtn} ${props.config.channels === "stereo" ? styles.channelActive : ""}`}
            onClick={() => props.onChange("channels", "stereo")}
            aria-pressed={props.config.channels === "stereo"}
          >
            Stereo
            <span class={styles.experimentalBadge}>experimentell</span>
          </button>
        </div>
      </div>
    </div>
  );
}
