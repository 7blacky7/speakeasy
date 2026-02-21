import { A } from "@solidjs/router";
import styles from "./Settings.module.css";

const SETTINGS_SECTIONS = [
  {
    path: "/settings/audio",
    label: "Audio",
    description: "Mikrofon, Lautsprecher und Rauschunterdrueckung konfigurieren",
  },
  {
    path: "/settings/account",
    label: "Account",
    description: "Passwort, Nickname und Away-Status verwalten",
  },
];

export default function Settings() {
  return (
    <div class={styles.page}>
      <h1 class={styles.title}>Einstellungen</h1>
      <div class={styles.sections}>
        {SETTINGS_SECTIONS.map((section) => (
          <A href={section.path} class={styles.sectionCard}>
            <div class={styles.sectionInfo}>
              <span class={styles.sectionTitle}>{section.label}</span>
              <span class={styles.sectionDesc}>{section.description}</span>
            </div>
            <span class={styles.arrow}>›</span>
          </A>
        ))}
      </div>
    </div>
  );
}
