import { A } from "@solidjs/router";
import styles from "./Sidebar.module.css";

export default function Sidebar() {
  return (
    <aside class={`${styles.sidebar} no-select`}>
      {/* Navigation - kompakt, kein Discord-Server-Icons-Stil */}
      <div class={styles.nav}>
        <A href="/" class={styles.navBtn} title="Server-Browser" activeClass={styles.active}>
          Server-Browser
        </A>
        <A href="/settings" class={styles.navBtn} title="Einstellungen" activeClass={styles.active}>
          Einstellungen
        </A>
        <A href="/admin" class={styles.navBtn} title="Server-Administration" activeClass={styles.active}>
          Administration
        </A>
      </div>
    </aside>
  );
}
