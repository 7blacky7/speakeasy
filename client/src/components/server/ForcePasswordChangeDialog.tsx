import { createSignal } from "solid-js";
import { changePassword } from "../../bridge";
import styles from "./ForcePasswordChangeDialog.module.css";

interface ForcePasswordChangeDialogProps {
  currentPassword: string;
  onPasswordChanged: () => void;
}

export default function ForcePasswordChangeDialog(
  props: ForcePasswordChangeDialogProps
) {
  const [newPassword, setNewPassword] = createSignal("");
  const [confirmPassword, setConfirmPassword] = createSignal("");
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);

  function validate(): string | null {
    const pw = newPassword().trim();
    if (pw.length < 8) {
      return "Das Passwort muss mindestens 8 Zeichen lang sein.";
    }
    if (pw.toLowerCase() === "admin") {
      return "Das Passwort darf nicht 'admin' sein.";
    }
    if (pw !== confirmPassword()) {
      return "Die Passwoerter stimmen nicht ueberein.";
    }
    return null;
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();
    const validationError = validate();
    if (validationError) {
      setError(validationError);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await changePassword(props.currentPassword, newPassword().trim());
      props.onPasswordChanged();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div class={styles.overlay}>
      <div class={styles.dialog}>
        <div class={styles.header}>
          <span class={styles.title}>Passwort aendern</span>
        </div>
        <div class={styles.body}>
          <p class={styles.hint}>
            Du verwendest noch das Standardpasswort. Bitte aendere es jetzt,
            bevor du den Server nutzen kannst.
          </p>
          <form id="force-pw-form" onSubmit={handleSubmit} class={styles.form}>
            <div class={styles.field}>
              <label class={styles.label} for="fpw-new">
                Neues Passwort
              </label>
              <input
                id="fpw-new"
                type="password"
                class={styles.input}
                value={newPassword()}
                onInput={(e) => setNewPassword(e.currentTarget.value)}
                placeholder="Mindestens 8 Zeichen"
                disabled={busy()}
                autofocus
              />
            </div>
            <div class={styles.field}>
              <label class={styles.label} for="fpw-confirm">
                Passwort bestaetigen
              </label>
              <input
                id="fpw-confirm"
                type="password"
                class={styles.input}
                value={confirmPassword()}
                onInput={(e) => setConfirmPassword(e.currentTarget.value)}
                placeholder="Passwort wiederholen"
                disabled={busy()}
              />
            </div>
            {error() && <div class={styles.errorMsg}>{error()}</div>}
          </form>
        </div>
        <div class={styles.actions}>
          <button
            type="submit"
            form="force-pw-form"
            class={styles.btnPrimary}
            disabled={busy() || newPassword().trim().length < 8}
          >
            {busy() ? "Wird geaendert..." : "Passwort aendern"}
          </button>
        </div>
      </div>
    </div>
  );
}
