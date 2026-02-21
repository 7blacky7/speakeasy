import { createSignal } from "solid-js";
import { toggleMute, toggleDeafen } from "../bridge";
import styles from "./Statusbar.module.css";

export default function Statusbar() {
  const [muted, setMuted] = createSignal(false);
  const [deafened, setDeafened] = createSignal(false);
  const [connected] = createSignal(true);

  async function handleToggleMute() {
    try {
      const result = await toggleMute();
      setMuted(result);
    } catch {
      setMuted((v) => !v);
    }
  }

  async function handleToggleDeafen() {
    try {
      const result = await toggleDeafen();
      setDeafened(result);
    } catch {
      setDeafened((v) => !v);
    }
  }

  return (
    <div class={`${styles.statusbar} no-select`}>
      {/* Verbindungsstatus + User-Info */}
      <div class={styles.userInfo}>
        <div class={styles.avatar}>
          <span>U</span>
          <span
            class={`${styles.statusDot} ${connected() ? styles.online : styles.offline}`}
          />
        </div>
        <div class={styles.userDetails}>
          <span class={styles.username}>Benutzer</span>
          <span class={styles.connectionStatus}>
            {connected() ? "Verbunden" : "Getrennt"}
          </span>
        </div>
      </div>

      {/* Audio-Controls */}
      <div class={styles.audioControls}>
        <button
          class={`${styles.audioBtn} ${muted() ? styles.active : ""}`}
          onClick={handleToggleMute}
          title={muted() ? "Stummschaltung aufheben" : "Stummschalten"}
        >
          {muted() ? "🔇" : "🎤"}
        </button>
        <button
          class={`${styles.audioBtn} ${deafened() ? styles.active : ""}`}
          onClick={handleToggleDeafen}
          title={deafened() ? "Ton einschalten" : "Ton ausschalten"}
        >
          {deafened() ? "🔕" : "🔊"}
        </button>
      </div>
    </div>
  );
}
