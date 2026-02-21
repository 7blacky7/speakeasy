import type { RouteSectionProps } from "@solidjs/router";
import styles from "./SubWindowLayout.module.css";

export default function SubWindowLayout(props: RouteSectionProps) {
  return (
    <div class={styles.root}>
      <main class={styles.main}>
        {props.children}
      </main>
    </div>
  );
}
