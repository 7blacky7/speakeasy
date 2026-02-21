import type { RouteSectionProps } from "@solidjs/router";
import Titlebar from "../components/Titlebar";
import Statusbar from "../components/Statusbar";
import styles from "./MainLayout.module.css";

export default function MainLayout(props: RouteSectionProps) {
  return (
    <div class={styles.root}>
      <Titlebar />
      <main class={styles.main}>
        {props.children}
      </main>
      <Statusbar />
    </div>
  );
}
