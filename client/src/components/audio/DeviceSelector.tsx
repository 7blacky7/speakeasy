import { For } from "solid-js";
import { AudioDevice } from "../../bridge";
import styles from "./DeviceSelector.module.css";

interface DeviceSelectorProps {
  label: string;
  kind: "input" | "output";
  devices: AudioDevice[];
  selectedId: string | null;
  onChange: (id: string) => void;
  onTest?: () => void;
}

export default function DeviceSelector(props: DeviceSelectorProps) {
  const filteredDevices = () =>
    props.devices.filter((d) => d.kind === props.kind);

  return (
    <div class={styles.row}>
      <label class={styles.label}>{props.label}</label>
      <div class={styles.controls}>
        <select
          class={styles.select}
          value={props.selectedId ?? ""}
          onChange={(e) => props.onChange(e.currentTarget.value)}
          aria-label={props.label}
        >
          <option value="">-- Standard --</option>
          <For each={filteredDevices()}>
            {(device) => (
              <option value={device.id}>
                {device.name}
                {device.is_default ? " (Standard)" : ""}
              </option>
            )}
          </For>
        </select>
        {props.onTest && (
          <button
            class={styles.testBtn}
            onClick={props.onTest}
            aria-label={`${props.label} testen`}
          >
            Test
          </button>
        )}
      </div>
    </div>
  );
}
