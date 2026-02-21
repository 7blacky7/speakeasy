import { A } from "@solidjs/router";
import styles from "./Sidebar.module.css";

export default function Sidebar() {
  return (
    <aside class={`${styles.sidebar} no-select`}>
      <div class={styles.nav}>
        <A href="/" class={styles.navBtn} title="Server-Browser" activeClass={styles.active} end>
          Server-Browser
        </A>
      </div>
    </aside>
  );
}
