import { createSignal, Show } from "solid-js";
import { adminUpdateServer } from "../../bridge";
import styles from "./ServerInfoPanel.module.css";

interface ServerInfoPanelProps {
  name: string;
  description: string;
  version: string;
  onlineClients: number;
  maxClients: number;
  isAdmin?: boolean;
  onServerUpdated?: () => void;
}

export default function ServerInfoPanel(props: ServerInfoPanelProps) {
  const [editName, setEditName] = createSignal("");
  const [editWelcome, setEditWelcome] = createSignal("");
  const [editMaxClients, setEditMaxClients] = createSignal(0);
  const [saving, setSaving] = createSignal(false);
  const [saveResult, setSaveResult] = createSignal<{ ok: boolean; msg: string } | null>(null);
  const [editing, setEditing] = createSignal(false);

  const startEditing = () => {
    setEditName(props.name);
    setEditWelcome(props.description);
    setEditMaxClients(props.maxClients);
    setSaveResult(null);
    setEditing(true);
  };

  const handleSave = async () => {
    setSaving(true);
    setSaveResult(null);
    try {
      await adminUpdateServer({
        name: editName(),
        willkommensnachricht: editWelcome(),
        max_clients: editMaxClients(),
      });
      setSaveResult({ ok: true, msg: "Gespeichert" });
      setEditing(false);
      props.onServerUpdated?.();
    } catch (e) {
      setSaveResult({ ok: false, msg: String(e) });
    } finally {
      setSaving(false);
    }
  };

  return (
    <div class={styles.panel}>
      <div class={styles.content}>
        <div class={styles.header}>
          <span class={styles.serverName}>{props.name}</span>
        </div>

        <Show when={props.description}>
          <div class={styles.section}>
            <div class={styles.sectionTitle}>Willkommen</div>
            <div class={styles.sectionContent}>{props.description}</div>
          </div>
        </Show>

        <div class={styles.section}>
          <div class={styles.sectionTitle}>Server-Details</div>
          <div class={styles.detailGrid}>
            <div class={styles.detailRow}>
              <span class={styles.detailLabel}>Version</span>
              <span class={styles.detailValue}>{props.version}</span>
            </div>
            <div class={styles.detailRow}>
              <span class={styles.detailLabel}>Clients</span>
              <span class={styles.detailValue}>
                {props.onlineClients}/{props.maxClients}
              </span>
            </div>
          </div>
        </div>

        {/* Admin-Bearbeitungsbereich */}
        <Show when={props.isAdmin}>
          <div class={styles.adminSection}>
            <div class={styles.adminTitle}>Administration</div>
            <Show
              when={editing()}
              fallback={
                <button class={styles.saveBtn} onClick={startEditing}>
                  Server bearbeiten
                </button>
              }
            >
              <div class={styles.formGroup}>
                <label class={styles.formLabel}>Server-Name</label>
                <input
                  class={styles.formInput}
                  type="text"
                  value={editName()}
                  onInput={(e) => setEditName(e.currentTarget.value)}
                />
              </div>
              <div class={styles.formGroup}>
                <label class={styles.formLabel}>Willkommensnachricht</label>
                <textarea
                  class={styles.formTextarea}
                  value={editWelcome()}
                  onInput={(e) => setEditWelcome(e.currentTarget.value)}
                />
              </div>
              <div class={styles.formGroup}>
                <label class={styles.formLabel}>Max. Clients</label>
                <input
                  class={styles.formInput}
                  type="number"
                  min="1"
                  value={editMaxClients()}
                  onInput={(e) => setEditMaxClients(parseInt(e.currentTarget.value) || 0)}
                />
              </div>
              <button class={styles.saveBtn} onClick={handleSave} disabled={saving()}>
                {saving() ? "Speichere..." : "Speichern"}
              </button>
            </Show>
            <Show when={saveResult()}>
              {(result) => (
                <div
                  class={`${styles.saveResult} ${result().ok ? styles.saveSuccess : styles.saveError}`}
                >
                  {result().msg}
                </div>
              )}
            </Show>
          </div>
        </Show>
      </div>
    </div>
  );
}
