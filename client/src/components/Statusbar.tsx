import { createSignal, onMount } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { toggleMute, toggleDeafen, disconnect, getCurrentUsername } from "../bridge";
import styles from "./Statusbar.module.css";

export default function Statusbar() {
  const [muted, setMuted] = createSignal(false);
  const [deafened, setDeafened] = createSignal(false);
  const [away, setAway] = createSignal(false);
  const [connected] = createSignal(true);
  const [username, setUsername] = createSignal<string | null>(null);
  const navigate = useNavigate();

  onMount(async () => {
    try {
      const name = await getCurrentUsername();
      setUsername(name);
    } catch {
      // kein Username verfuegbar
    }
  });

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

  function handleToggleAway() {
    setAway((v) => !v);
  }

  async function handleDisconnect() {
    try {
      await disconnect();
      navigate("/");
    } catch (e) {
      console.error("Trennen fehlgeschlagen:", e);
    }
  }

  return (
    <div class={`${styles.statusbar} no-select`}>
      {/* User-Info */}
      <div class={styles.userInfo}>
        <span
          class={`${styles.statusDot} ${connected() ? styles.online : styles.offline}`}
        />
        <span class={styles.username}>{username() ?? "Benutzer"}</span>
      </div>

      {/* Audio-Controls */}
      <div class={styles.controls}>
        <button
          class={`${styles.controlBtn} ${muted() ? styles.active : ""}`}
          onClick={handleToggleMute}
          title={muted() ? "Stummschaltung aufheben" : "Stummschalten"}
        >
          {muted() ? "MIC AUS" : "MIC"}
        </button>
        <button
          class={`${styles.controlBtn} ${deafened() ? styles.active : ""}`}
          onClick={handleToggleDeafen}
          title={deafened() ? "Ton einschalten" : "Ton ausschalten"}
        >
          {deafened() ? "TON AUS" : "TON"}
        </button>
        <button
          class={`${styles.controlBtn} ${away() ? styles.away : ""}`}
          onClick={handleToggleAway}
          title={away() ? "Away-Status aufheben" : "Away setzen"}
        >
          AFK
        </button>

        <span class={styles.separator} />

        <button
          class={`${styles.controlBtn} ${styles.disconnectBtn}`}
          onClick={handleDisconnect}
          title="Verbindung trennen"
        >
          TRENNEN
        </button>
      </div>
    </div>
  );
}
