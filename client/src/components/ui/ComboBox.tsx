import { createSignal, createMemo, For, Show, onCleanup, createEffect } from "solid-js";
import styles from "./ComboBox.module.css";

interface ComboBoxProps {
  value: string;
  suggestions: string[];
  onChange: (value: string) => void;
  onSelect?: (value: string) => void;
  placeholder?: string;
  ariaLabel?: string;
  class?: string;
}

export default function ComboBox(props: ComboBoxProps) {
  const [open, setOpen] = createSignal(false);
  const [focusedIdx, setFocusedIdx] = createSignal(-1);
  let wrapperRef!: HTMLDivElement;
  let inputRef!: HTMLInputElement;

  const filtered = createMemo(() => {
    const query = props.value.toLowerCase().trim();
    if (!query) return props.suggestions;
    return props.suggestions.filter((s) => s.toLowerCase().includes(query));
  });

  function openDropdown() {
    setOpen(true);
    setFocusedIdx(-1);
  }

  function closeDropdown() {
    setOpen(false);
    setFocusedIdx(-1);
  }

  function selectItem(value: string) {
    props.onChange(value);
    props.onSelect?.(value);
    closeDropdown();
    inputRef?.focus();
  }

  function toggleDropdown() {
    if (open()) {
      closeDropdown();
    } else {
      openDropdown();
    }
  }

  function handleInput(e: InputEvent) {
    props.onChange((e.currentTarget as HTMLInputElement).value);
    if (!open()) {
      openDropdown();
    }
    setFocusedIdx(-1);
  }

  function handleKeyDown(e: KeyboardEvent) {
    const items = filtered();

    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        if (!open()) {
          openDropdown();
        } else {
          setFocusedIdx((i) => Math.min(i + 1, items.length - 1));
        }
        break;
      case "ArrowUp":
        e.preventDefault();
        if (open()) {
          setFocusedIdx((i) => Math.max(i - 1, 0));
        }
        break;
      case "Enter":
        if (open() && focusedIdx() >= 0 && focusedIdx() < items.length) {
          e.preventDefault();
          selectItem(items[focusedIdx()]);
        }
        break;
      case "Escape":
        if (open()) {
          e.preventDefault();
          closeDropdown();
        }
        break;
      case "Tab":
        closeDropdown();
        break;
    }
  }

  // Close on outside click
  function handleClickOutside(e: MouseEvent) {
    if (open() && wrapperRef && !wrapperRef.contains(e.target as Node)) {
      closeDropdown();
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
    <div ref={wrapperRef} class={`${styles.wrapper} ${props.class ?? ""}`}>
      <div class={`${styles.inputRow} ${open() ? styles.open : ""}`}>
        <input
          ref={inputRef}
          type="text"
          class={styles.input}
          value={props.value}
          onInput={handleInput}
          onKeyDown={handleKeyDown}
          onFocus={() => {
            if (props.suggestions.length > 0) openDropdown();
          }}
          placeholder={props.placeholder}
          aria-label={props.ariaLabel}
          aria-haspopup="listbox"
          aria-expanded={open()}
          autocomplete="off"
        />
        <Show when={props.suggestions.length > 0}>
          <button
            type="button"
            class={styles.toggleBtn}
            onClick={toggleDropdown}
            tabIndex={-1}
            aria-label="Vorschlaege anzeigen"
          >
            <svg
              class={`${styles.arrow} ${open() ? styles.arrowOpen : ""}`}
              viewBox="0 0 12 12"
              fill="none"
            >
              <path
                d="M2 4l4 4 4-4"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </button>
        </Show>
      </div>

      <Show when={open() && filtered().length > 0}>
        <div class={styles.dropdown} role="listbox">
          <For each={filtered()}>
            {(item, idx) => (
              <button
                type="button"
                class={`${styles.option} ${item === props.value ? styles.selected : ""} ${idx() === focusedIdx() ? styles.focused : ""}`}
                data-idx={idx()}
                onClick={() => selectItem(item)}
                onMouseEnter={() => setFocusedIdx(idx())}
                role="option"
                aria-selected={item === props.value}
              >
                {item}
              </button>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
