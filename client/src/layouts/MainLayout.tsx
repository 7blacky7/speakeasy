import type { RouteSectionProps } from "@solidjs/router";
import Titlebar from "../components/Titlebar";
import Sidebar from "../components/Sidebar";
import Statusbar from "../components/Statusbar";
import styles from "./MainLayout.module.css";

export default function MainLayout(props: RouteSectionProps) {
  return (
    <div class={styles.root}>
      <Titlebar />
      <div class={styles.body}>
        <Sidebar />
        <main class={styles.main}>
          {props.children}
        </main>
      </div>
      <Statusbar />
    </div>
  );
}
