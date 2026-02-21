import { A } from "@solidjs/router";
import styles from "./AudioSettings.module.css";

export default function AudioSettings() {
  return (
    <div class={styles.page}>
      <div class={styles.breadcrumb}>
        <A href="/settings" class={styles.breadcrumbLink}>
          Einstellungen
        </A>
        <span class={styles.breadcrumbSep}>›</span>
        <span>Audio</span>
      </div>

      <h1 class={styles.title}>Audio-Einstellungen</h1>

      <div class={styles.placeholder}>
        <span class={styles.placeholderIcon}>🎙️</span>
        <p class={styles.placeholderText}>
          Audio-Einstellungen werden in Phase 3 implementiert.
        </p>
        <p class={styles.placeholderSub}>
          Enthält: Gerätewahl, Lautstärke, Rauschunterdrückung, Echokompensation
        </p>
      </div>
    </div>
  );
}
