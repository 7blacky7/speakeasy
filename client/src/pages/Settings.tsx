import { A } from "@solidjs/router";
import styles from "./Settings.module.css";

const SETTINGS_SECTIONS = [
  {
    path: "/settings/audio",
    icon: "🎙️",
    title: "Audio",
    description: "Mikrofon, Lautsprecher und Rauschunterdrückung konfigurieren",
  },
];

export default function Settings() {
  return (
    <div class={styles.page}>
      <h1 class={styles.title}>Einstellungen</h1>
      <div class={styles.sections}>
        {SETTINGS_SECTIONS.map((section) => (
          <A href={section.path} class={styles.sectionCard}>
            <span class={styles.sectionIcon}>{section.icon}</span>
            <div class={styles.sectionInfo}>
              <span class={styles.sectionTitle}>{section.title}</span>
              <span class={styles.sectionDesc}>{section.description}</span>
            </div>
            <span class={styles.arrow}>›</span>
          </A>
        ))}
      </div>
    </div>
  );
}
