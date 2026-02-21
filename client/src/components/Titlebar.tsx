import { getCurrentWindow } from "@tauri-apps/api/window";
import styles from "./Titlebar.module.css";

export default function Titlebar() {
  const appWindow = getCurrentWindow();

  return (
    <div class={`${styles.titlebar} no-select`} data-tauri-drag-region>
      <div class={styles.appName} data-tauri-drag-region>
        <span class={styles.logo}>▶</span>
        <span>Speakeasy</span>
      </div>
      <div class={styles.controls}>
        <button
          class={styles.controlBtn}
          onClick={() => appWindow.minimize()}
          title="Minimieren"
        >
          ─
        </button>
        <button
          class={styles.controlBtn}
          onClick={() => appWindow.toggleMaximize()}
          title="Maximieren"
        >
          □
        </button>
        <button
          class={`${styles.controlBtn} ${styles.closeBtn}`}
          onClick={() => appWindow.close()}
          title="Schließen"
        >
          ✕
        </button>
      </div>
    </div>
  );
}
