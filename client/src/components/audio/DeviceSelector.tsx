import { AudioDevice } from "../../bridge";
import CustomSelect from "../ui/CustomSelect";
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

  const options = () => [
    { value: "", label: "-- Standard --" },
    ...filteredDevices().map((d) => ({
      value: d.id,
      label: d.name + (d.is_default ? " (Standard)" : ""),
    })),
  ];

  return (
    <div class={styles.row}>
      <label class={styles.label}>{props.label}</label>
      <div class={styles.controls}>
        <CustomSelect
          value={props.selectedId ?? ""}
          options={options()}
          onChange={props.onChange}
          ariaLabel={props.label}
        />
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
