import { createSignal, For, Show, onCleanup, createEffect } from "solid-js";
import styles from "./CustomSelect.module.css";

export interface SelectOption {
  value: string;
  label: string;
}

interface CustomSelectProps {
  value: string;
  options: SelectOption[];
  onChange: (value: string) => void;
  placeholder?: string;
  class?: string;
  ariaLabel?: string;
}

export default function CustomSelect(props: CustomSelectProps) {
  const [open, setOpen] = createSignal(false);
  const [focusedIdx, setFocusedIdx] = createSignal(-1);
  let wrapperRef!: HTMLDivElement;

  const selectedLabel = () => {
    const opt = props.options.find((o) => o.value === props.value);
    return opt?.label;
  };

  function toggle() {
    const next = !open();
    setOpen(next);
    if (next) {
      const idx = props.options.findIndex((o) => o.value === props.value);
      setFocusedIdx(idx >= 0 ? idx : 0);
    }
  }

  function select(value: string) {
    props.onChange(value);
    setOpen(false);
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (!open()) {
      if (e.key === "Enter" || e.key === " " || e.key === "ArrowDown") {
        e.preventDefault();
        toggle();
      }
      return;
    }

    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setFocusedIdx((i) => Math.min(i + 1, props.options.length - 1));
        break;
      case "ArrowUp":
        e.preventDefault();
        setFocusedIdx((i) => Math.max(i - 1, 0));
        break;
      case "Enter":
      case " ":
        e.preventDefault();
        if (focusedIdx() >= 0 && focusedIdx() < props.options.length) {
          select(props.options[focusedIdx()].value);
        }
        break;
      case "Escape":
        e.preventDefault();
        setOpen(false);
        break;
    }
  }

  // Close on outside click
  function handleClickOutside(e: MouseEvent) {
    if (open() && wrapperRef && !wrapperRef.contains(e.target as Node)) {
      setOpen(false);
    }
  }

  createEffect(() => {
    if (open()) {
      document.addEventListener("mousedown", handleClickOutside);
    } else {
      document.removeEventListener("mousedown", handleClickOutside);
    }
  });

  onCleanup(() => {
    document.removeEventListener("mousedown", handleClickOutside);
  });

  // Scroll focused option into view
  createEffect(() => {
    const idx = focusedIdx();
    if (open() && idx >= 0) {
      const el = wrapperRef?.querySelector(`[data-idx="${idx}"]`);
      el?.scrollIntoView({ block: "nearest" });
    }
  });

  return (
    <div
      ref={wrapperRef}
      class={`${styles.wrapper} ${props.class ?? ""}`}
    >
      <button
        type="button"
        class={`${styles.trigger} ${open() ? styles.open : ""}`}
        onClick={toggle}
        onKeyDown={handleKeyDown}
        aria-haspopup="listbox"
        aria-expanded={open()}
        aria-label={props.ariaLabel}
      >
        <span class={`${styles.triggerLabel} ${!selectedLabel() ? styles.triggerPlaceholder : ""}`}>
          {selectedLabel() ?? props.placeholder ?? "Auswählen..."}
        </span>
        <svg class={`${styles.arrow} ${open() ? styles.arrowOpen : ""}`} viewBox="0 0 12 12" fill="none">
          <path d="M2 4l4 4 4-4" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </button>

      <Show when={open()}>
        <div class={styles.dropdown} role="listbox">
          <For each={props.options}>
            {(opt, idx) => (
              <button
                type="button"
                class={`${styles.option} ${opt.value === props.value ? styles.selected : ""} ${idx() === focusedIdx() ? styles.focused : ""}`}
                data-idx={idx()}
                onClick={() => select(opt.value)}
                onMouseEnter={() => setFocusedIdx(idx())}
                role="option"
                aria-selected={opt.value === props.value}
              >
                {opt.label}
              </button>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
