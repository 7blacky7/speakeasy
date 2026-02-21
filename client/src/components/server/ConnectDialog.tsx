import { createSignal, Show } from "solid-js";
import { connectToServer, clearForcePasswordChange } from "../../bridge";
import { loadHistory, saveConnection, findEntry } from "../../utils/connectionHistory";
import ForcePasswordChangeDialog from "./ForcePasswordChangeDialog";
import ComboBox from "../ui/ComboBox";
import Modal from "../ui/Modal";
import styles from "./ConnectDialog.module.css";

const STORAGE_KEY_ADDRESS = "speakeasy_last_address";
const STORAGE_KEY_PORT = "speakeasy_last_port";
const STORAGE_KEY_USERNAME = "speakeasy_last_username";

interface ConnectDialogProps {
  onClose: () => void;
  onConnected: (details?: { address: string; port: number; username: string; password?: string }) => void;
}

export default function ConnectDialog(props: ConnectDialogProps) {
  const history = loadHistory();

  const [address, setAddress] = createSignal(
    localStorage.getItem(STORAGE_KEY_ADDRESS) || "localhost"
  );
  const [port, setPort] = createSignal(
    Number(localStorage.getItem(STORAGE_KEY_PORT) || "9001")
  );
  const [username, setUsername] = createSignal(
    localStorage.getItem(STORAGE_KEY_USERNAME) || ""
  );
  const [password, setPassword] = createSignal("");
  const [connecting, setConnecting] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [showPasswordChange, setShowPasswordChange] = createSignal(false);

  async function handleConnect(e: Event) {
    e.preventDefault();
    if (!username()) {
      setError("Bitte einen Benutzernamen eingeben.");
      return;
    }
    setError(null);
    setConnecting(true);
    try {
      const result = await connectToServer({
        address: address(),
        port: port(),
        username: username(),
        password: password() || undefined,
      });
      localStorage.setItem(STORAGE_KEY_ADDRESS, address());
      localStorage.setItem(STORAGE_KEY_PORT, String(port()));
      localStorage.setItem(STORAGE_KEY_USERNAME, username());
      saveConnection(address(), port(), username());
      const details = { address: address(), port: port(), username: username(), password: password() || undefined };
      if (result.must_change_password) {
        setShowPasswordChange(true);
      } else {
        props.onConnected(details);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setConnecting(false);
    }
  }

  async function handlePasswordChanged() {
    await clearForcePasswordChange();
    setShowPasswordChange(false);
    props.onConnected({ address: address(), port: port(), username: username(), password: password() || undefined });
  }

  const actions = (
    <>
      <button
        type="button"
        class={styles.btnCancel}
        onClick={props.onClose}
        disabled={connecting()}
      >
        Abbrechen
      </button>
      <button
        type="submit"
        form="connect-dialog-form"
        class={styles.btnPrimary}
        disabled={connecting() || !username()}
      >
        {connecting() ? "Verbinde..." : "Verbinden"}
      </button>
    </>
  );

  return (
    <>
      <Show when={showPasswordChange()}>
        <ForcePasswordChangeDialog
          onPasswordChanged={handlePasswordChanged}
          currentPassword={password()}
        />
      </Show>
      <Modal title="Mit Server verbinden" onClose={props.onClose} actions={actions}>
        <form id="connect-dialog-form" onSubmit={handleConnect} class={styles.form}>
          <div class={styles.row}>
            <div class={styles.field} style={{ flex: "1" }}>
              <label class={styles.label} for="cd-address">
                Adresse
              </label>
              <ComboBox
                value={address()}
                suggestions={history.map((e) => e.address)}
                onChange={setAddress}
                onSelect={(addr) => {
                  const entry = findEntry(addr);
                  if (entry) {
                    setPort(entry.port);
                    setUsername(entry.username);
                  }
                }}
                placeholder="Serveradresse"
                ariaLabel="Server-Adresse"
              />
            </div>
            <div class={`${styles.field} ${styles.portField}`}>
              <label class={styles.label} for="cd-port">
                Port
              </label>
              <input
                id="cd-port"
                type="number"
                class={styles.input}
                value={port()}
                onInput={(e) => setPort(Number(e.currentTarget.value))}
                min={1}
                max={65535}
                disabled={connecting()}
              />
            </div>
          </div>

          <div class={styles.field}>
            <label class={styles.label} for="cd-username">
              Benutzername
            </label>
            <input
              id="cd-username"
              type="text"
              class={styles.input}
              value={username()}
              onInput={(e) => setUsername(e.currentTarget.value)}
              placeholder="Dein Name"
              disabled={connecting()}
              required
            />
          </div>

          <div class={styles.field}>
            <label class={styles.label} for="cd-password">
              Passwort (optional)
            </label>
            <input
              id="cd-password"
              type="password"
              class={styles.input}
              value={password()}
              onInput={(e) => setPassword(e.currentTarget.value)}
              placeholder="Leer lassen falls kein Passwort"
              disabled={connecting()}
            />
          </div>

          {error() && <div class={styles.errorMsg}>{error()}</div>}
        </form>
      </Modal>
    </>
  );
}
