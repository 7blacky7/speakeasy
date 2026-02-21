import type { RouteSectionProps } from "@solidjs/router";
import Titlebar from "../components/Titlebar";
import styles from "./SubWindowLayout.module.css";

export default function SubWindowLayout(props: RouteSectionProps) {
  return (
    <div class={styles.root}>
      <Titlebar />
      <main class={styles.main}>
        {props.children}
      </main>
    </div>
  );
}
