import { createSignal, onMount } from "solid-js";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import styles from "./Titlebar.module.css";

export default function Titlebar() {
  const appWindow = getCurrentWebviewWindow();
  const [title, setTitle] = createSignal("Speakeasy");

  onMount(async () => {
    try {
      const winTitle = await appWindow.title();
      if (winTitle) {
        setTitle(winTitle);
      }
    } catch {
      // Fallback
    }
  });

  async function handleMinimize() {
    try {
      await appWindow.minimize();
    } catch (e) {
      console.error("Window minimize failed:", e);
    }
  }

  async function handleToggleMaximize() {
    try {
      await appWindow.toggleMaximize();
    } catch (e) {
      console.error("Window toggleMaximize failed:", e);
    }
  }

  async function handleClose() {
    try {
      await appWindow.close();
    } catch (e) {
      console.error("Window close failed:", e);
    }
  }

  async function handleDragStart(e: MouseEvent) {
    // Only drag on primary mouse button and not on buttons
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest("button")) return;
    try {
      await appWindow.startDragging();
    } catch (e) {
      console.error("Window startDragging failed:", e);
    }
  }

  return (
    <div
      class={`${styles.titlebar} no-select`}
      data-tauri-drag-region
      onMouseDown={handleDragStart}
    >
      <div class={styles.appName} data-tauri-drag-region>
        <span class={styles.logo}>▶</span>
        <span>{title()}</span>
      </div>
      <div class={styles.controls}>
        <button
          class={styles.controlBtn}
          onClick={handleMinimize}
          title="Minimieren"
        >
          ─
        </button>
        <button
          class={styles.controlBtn}
          onClick={handleToggleMaximize}
          title="Maximieren"
        >
          □
        </button>
        <button
          class={`${styles.controlBtn} ${styles.closeBtn}`}
          onClick={handleClose}
          title="Schließen"
        >
          ✕
        </button>
      </div>
    </div>
  );
}
