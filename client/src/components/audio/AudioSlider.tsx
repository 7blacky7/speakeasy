import styles from "./AudioSlider.module.css";

interface AudioSliderProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  onChange: (value: number) => void;
  showInput?: boolean;
}

export default function AudioSlider(props: AudioSliderProps) {
  const percentage = () =>
    ((props.value - props.min) / (props.max - props.min)) * 100;

  return (
    <div class={styles.container}>
      <div class={styles.header}>
        <label class={styles.label}>{props.label}</label>
        <span class={styles.value}>
          {props.showInput ? (
            <input
              type="number"
              class={styles.numInput}
              value={props.value}
              min={props.min}
              max={props.max}
              step={props.step ?? 1}
              onInput={(e) => {
                const v = parseFloat(e.currentTarget.value);
                if (!isNaN(v))
                  props.onChange(Math.min(props.max, Math.max(props.min, v)));
              }}
              aria-label={props.label}
            />
          ) : (
            `${props.value}${props.unit ?? ""}`
          )}
        </span>
      </div>
      <div class={styles.track}>
        <div
          class={styles.fill}
          style={{ width: `${percentage()}%` }}
        />
        <input
          type="range"
          class={styles.slider}
          min={props.min}
          max={props.max}
          step={props.step ?? 1}
          value={props.value}
          onInput={(e) =>
            props.onChange(parseFloat(e.currentTarget.value))
          }
          aria-label={props.label}
          aria-valuemin={props.min}
          aria-valuemax={props.max}
          aria-valuenow={props.value}
        />
      </div>
    </div>
  );
}
