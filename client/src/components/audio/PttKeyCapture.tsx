import { createSignal, onCleanup, onMount } from "solid-js";
import styles from "./PttKeyCapture.module.css";

interface PttKeyCaptureProps {
  currentKey: string | null;
  onConfirm: (key: string) => void;
  onCancel: () => void;
}

/** Wandelt einen KeyboardEvent in einen lesbaren Tastennamen um */
function formatKeyName(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.ctrlKey && e.key !== "Control") parts.push("Strg");
  if (e.altKey && e.key !== "Alt") parts.push("Alt");
  if (e.shiftKey && e.key !== "Shift") parts.push("Shift");
  if (e.metaKey && e.key !== "Meta") parts.push("Meta");

  const keyMap: Record<string, string> = {
    " ": "Leertaste",
    Control: "Strg",
    Meta: "Meta",
    Escape: "Esc",
    ArrowUp: "Pfeil hoch",
    ArrowDown: "Pfeil runter",
    ArrowLeft: "Pfeil links",
    ArrowRight: "Pfeil rechts",
    Backspace: "Ruecktaste",
    Delete: "Entf",
    Insert: "Einfg",
    CapsLock: "Feststelltaste",
    Tab: "Tab",
    Enter: "Eingabe",
  };

  const name = keyMap[e.key] ?? e.key.toUpperCase();

  // Nur Modifier allein -> direkt als Taste nehmen
  if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) {
    return name;
  }

  parts.push(name);
  return parts.join(" + ");
}

export default function PttKeyCapture(props: PttKeyCaptureProps) {
  const [capturedKey, setCapturedKey] = createSignal<string | null>(null);

  function handleKeyDown(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();

    // Escape schließt den Dialog, ohne eine Taste zu setzen
    if (e.key === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey) {
      props.onCancel();
      return;
    }

    setCapturedKey(formatKeyName(e));
  }

  onMount(() => {
    window.addEventListener("keydown", handleKeyDown, true);
  });

  onCleanup(() => {
    window.removeEventListener("keydown", handleKeyDown, true);
  });

  return (
    <div class={styles.overlay} onClick={(e) => { if (e.target === e.currentTarget) props.onCancel(); }}>
      <div class={styles.modal}>
        <div class={styles.header}>
          <span class={styles.title}>PTT-Taste festlegen</span>
        </div>

        <div class={styles.body}>
          <p class={styles.prompt}>
            Druecke die gewuenschte Taste fuer Push-to-Talk:
          </p>

          <div class={`${styles.keyDisplay} ${capturedKey() == null ? styles.keyDisplayWaiting : ""}`}>
            {capturedKey() ?? "Warte auf Tastendruck..."}
          </div>

          <p class={styles.hint}>
            Esc zum Abbrechen. Modifier-Kombinationen (Strg+X) werden unterstuetzt.
          </p>
        </div>

        <div class={styles.footer}>
          <button class={styles.cancelBtn} onClick={props.onCancel}>
            Abbrechen
          </button>
          <button
            class={styles.confirmBtn}
            disabled={capturedKey() == null}
            onClick={() => {
              const key = capturedKey();
              if (key) props.onConfirm(key);
            }}
          >
            Uebernehmen
          </button>
        </div>
      </div>
    </div>
  );
}
