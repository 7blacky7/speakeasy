import styles from "./ModeSwitch.module.css";

interface ModeSwitchProps {
  mode: "simple" | "expert";
  onChange: (mode: "simple" | "expert") => void;
}

export default function ModeSwitch(props: ModeSwitchProps) {
  return (
    <div class={styles.container} role="tablist" aria-label="Einstellungsmodus">
      <button
        role="tab"
        aria-selected={props.mode === "simple"}
        class={`${styles.tab} ${props.mode === "simple" ? styles.active : ""}`}
        onClick={() => props.onChange("simple")}
      >
        Einfach
      </button>
      <button
        role="tab"
        aria-selected={props.mode === "expert"}
        class={`${styles.tab} ${props.mode === "expert" ? styles.active : ""}`}
        onClick={() => props.onChange("expert")}
      >
        Experte
      </button>
    </div>
  );
}
