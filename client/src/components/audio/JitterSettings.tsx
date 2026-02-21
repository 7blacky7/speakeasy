import { JitterConfig } from "../../bridge";
import AudioSlider from "./AudioSlider";
import DspModule from "./DspModule";
import styles from "./JitterSettings.module.css";

interface JitterSettingsProps {
  config: JitterConfig;
  onChange: <K extends keyof JitterConfig>(key: K, value: JitterConfig[K]) => void;
}

export default function JitterSettings(props: JitterSettingsProps) {
  return (
    <div class={styles.container}>
      <AudioSlider
        label="Minimaler Puffer"
        value={props.config.minBuffer}
        min={0}
        max={200}
        step={5}
        unit=" ms"
        onChange={(v) => props.onChange("minBuffer", v)}
      />
      <AudioSlider
        label="Maximaler Puffer"
        value={props.config.maxBuffer}
        min={20}
        max={500}
        step={10}
        unit=" ms"
        onChange={(v) => props.onChange("maxBuffer", v)}
      />
      <DspModule
        label="Adaptiver Jitter Buffer"
        enabled={props.config.adaptive}
        onToggle={(v) => props.onChange("adaptive", v)}
        tooltip="Passt den Puffer automatisch an Netzwerkbedingungen an"
      />
    </div>
  );
}
