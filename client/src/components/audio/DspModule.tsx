import { Show, JSX } from "solid-js";
import styles from "./DspModule.module.css";

interface DspModuleProps {
  label: string;
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
  children?: JSX.Element;
  tooltip?: string;
}

export default function DspModule(props: DspModuleProps) {
  const id = () => `dsp-toggle-${props.label.toLowerCase().replace(/\s+/g, "-")}`;

  return (
    <div class={`${styles.module} ${props.enabled ? styles.enabled : ""}`}>
      <div class={styles.header}>
        <div class={styles.headerLeft}>
          <label class={styles.label} for={id()}>
            {props.label}
          </label>
          {props.tooltip && (
            <span class={styles.tooltip} title={props.tooltip}>?</span>
          )}
        </div>
        <button
          id={id()}
          role="switch"
          aria-checked={props.enabled}
          class={`${styles.toggle} ${props.enabled ? styles.toggleOn : ""}`}
          onClick={() => props.onToggle(!props.enabled)}
          aria-label={`${props.label} ${props.enabled ? "deaktivieren" : "aktivieren"}`}
        >
          <span class={styles.toggleThumb} />
        </button>
      </div>
      <Show when={props.enabled && props.children}>
        <div class={styles.content}>{props.children}</div>
      </Show>
    </div>
  );
}
