import { createSignal } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { connectToServer } from "../bridge";
import styles from "./ServerBrowser.module.css";

export default function ServerBrowser() {
  const navigate = useNavigate();
  const [address, setAddress] = createSignal("localhost");
  const [port, setPort] = createSignal(9001);
  const [username, setUsername] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [connecting, setConnecting] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);

  async function handleConnect(e: Event) {
    e.preventDefault();
    if (!username()) {
      setError("Bitte einen Benutzernamen eingeben.");
      return;
    }
    setError(null);
    setConnecting(true);
    try {
      await connectToServer({
        address: address(),
        port: port(),
        username: username(),
        password: password() || undefined,
      });
      navigate("/server/1");
    } catch (err) {
      setError(String(err));
    } finally {
      setConnecting(false);
    }
  }

  return (
    <div class={styles.page}>
      <div class={styles.hero}>
        <h1 class={styles.title}>Speakeasy</h1>
        <p class={styles.subtitle}>Open-Source Voice-Chat</p>
      </div>

      <div class={styles.connectCard}>
        <h2 class={styles.cardTitle}>Mit Server verbinden</h2>
        <form class={styles.form} onSubmit={handleConnect}>
          <div class={styles.row}>
            <label class={styles.label}>
              Adresse
              <input
                class={styles.input}
                type="text"
                value={address()}
                onInput={(e) => setAddress(e.currentTarget.value)}
                placeholder="z.B. localhost oder 192.168.1.1"
                disabled={connecting()}
              />
            </label>
            <label class={`${styles.label} ${styles.portLabel}`}>
              Port
              <input
                class={styles.input}
                type="number"
                value={port()}
                onInput={(e) => setPort(Number(e.currentTarget.value))}
                min={1}
                max={65535}
                disabled={connecting()}
              />
            </label>
          </div>

          <label class={styles.label}>
            Benutzername
            <input
              class={styles.input}
              type="text"
              value={username()}
              onInput={(e) => setUsername(e.currentTarget.value)}
              placeholder="Dein Name"
              disabled={connecting()}
              required
            />
          </label>

          <label class={styles.label}>
            Passwort (optional)
            <input
              class={styles.input}
              type="password"
              value={password()}
              onInput={(e) => setPassword(e.currentTarget.value)}
              placeholder="Leer lassen falls kein Passwort"
              disabled={connecting()}
            />
          </label>

          {error() && <p class={styles.error}>{error()}</p>}

          <button class={styles.connectBtn} type="submit" disabled={connecting()}>
            {connecting() ? "Verbinde..." : "Verbinden"}
          </button>
        </form>
      </div>
    </div>
  );
}
