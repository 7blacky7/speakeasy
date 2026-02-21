import { A } from "@solidjs/router";
import { createSignal } from "solid-js";
import { changeNickname, changePassword, setAway } from "../bridge";
import styles from "./AccountSettings.module.css";

// --- Passwort aendern ---

function PasswordSection() {
  const [oldPassword, setOldPassword] = createSignal("");
  const [newPassword, setNewPassword] = createSignal("");
  const [confirm, setConfirm] = createSignal("");
  const [error, setError] = createSignal("");
  const [success, setSuccess] = createSignal("");
  const [busy, setBusy] = createSignal(false);

  const validationError = () => {
    if (newPassword().length > 0 && newPassword().length < 6) {
      return "Neues Passwort muss mindestens 6 Zeichen lang sein";
    }
    if (confirm().length > 0 && newPassword() !== confirm()) {
      return "Passwoerter stimmen nicht ueberein";
    }
    return "";
  };

  const canSubmit = () =>
    !busy() &&
    oldPassword().length > 0 &&
    newPassword().length >= 6 &&
    newPassword() === confirm() &&
    validationError() === "";

  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (!canSubmit()) return;
    setBusy(true);
    setError("");
    setSuccess("");
    try {
      await changePassword(oldPassword(), newPassword());
      setSuccess("Passwort erfolgreich geaendert");
      setOldPassword("");
      setNewPassword("");
      setConfirm("");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section class={styles.section}>
      <div class={styles.sectionTitle}>Passwort aendern</div>
      <form class={styles.sectionBody} onSubmit={handleSubmit}>
        <div class={styles.fieldRow}>
          <label class={styles.label}>Aktuelles Passwort</label>
          <input
            type="password"
            class={styles.input}
            value={oldPassword()}
            onInput={(e) => setOldPassword(e.currentTarget.value)}
            disabled={busy()}
            autocomplete="current-password"
          />
        </div>
        <div class={styles.fieldRow}>
          <label class={styles.label}>Neues Passwort</label>
          <input
            type="password"
            class={`${styles.input}${validationError() && newPassword().length > 0 ? " " + styles.inputError : ""}`}
            value={newPassword()}
            onInput={(e) => setNewPassword(e.currentTarget.value)}
            disabled={busy()}
            autocomplete="new-password"
          />
        </div>
        <div class={styles.fieldRow}>
          <label class={styles.label}>Neues Passwort bestaetigen</label>
          <input
            type="password"
            class={`${styles.input}${validationError() && confirm().length > 0 ? " " + styles.inputError : ""}`}
            value={confirm()}
            onInput={(e) => setConfirm(e.currentTarget.value)}
            disabled={busy()}
            autocomplete="new-password"
          />
          {validationError() && (
            <span class={styles.errorText}>{validationError()}</span>
          )}
        </div>
        <div class={styles.btnRow}>
          <button type="submit" class={styles.btnPrimary} disabled={!canSubmit()}>
            {busy() ? "Wird gespeichert..." : "Passwort speichern"}
          </button>
          {error() && <span class={styles.errorText}>{error()}</span>}
          {success() && <span class={styles.successText}>{success()}</span>}
        </div>
      </form>
    </section>
  );
}

// --- Nickname aendern ---

function NicknameSection() {
  const [nickname, setNickname] = createSignal("");
  const [error, setError] = createSignal("");
  const [success, setSuccess] = createSignal("");
  const [busy, setBusy] = createSignal(false);

  const canSubmit = () =>
    !busy() && nickname().trim().length > 0 && nickname().trim().length <= 64;

  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (!canSubmit()) return;
    setBusy(true);
    setError("");
    setSuccess("");
    try {
      const gesetzt = await changeNickname(nickname().trim());
      setSuccess(`Nickname geaendert zu: ${gesetzt}`);
      setNickname("");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section class={styles.section}>
      <div class={styles.sectionTitle}>Nickname aendern</div>
      <form class={styles.sectionBody} onSubmit={handleSubmit}>
        <div class={styles.fieldRow}>
          <label class={styles.label}>Neuer Anzeigename</label>
          <input
            type="text"
            class={styles.input}
            value={nickname()}
            onInput={(e) => setNickname(e.currentTarget.value)}
            disabled={busy()}
            maxlength={64}
            placeholder="Neuer Nickname..."
          />
        </div>
        <div class={styles.btnRow}>
          <button type="submit" class={styles.btnPrimary} disabled={!canSubmit()}>
            {busy() ? "Wird gespeichert..." : "Nickname speichern"}
          </button>
          {error() && <span class={styles.errorText}>{error()}</span>}
          {success() && <span class={styles.successText}>{success()}</span>}
        </div>
      </form>
    </section>
  );
}

// --- Away-Status ---

function AwaySection() {
  const [away, setAwaySig] = createSignal(false);
  const [awayMessage, setAwayMessage] = createSignal("");
  const [error, setError] = createSignal("");
  const [success, setSuccess] = createSignal("");
  const [busy, setBusy] = createSignal(false);

  async function handleToggle() {
    const neuerWert = !away();
    setBusy(true);
    setError("");
    setSuccess("");
    try {
      await setAway(neuerWert, awayMessage() || undefined);
      setAwaySig(neuerWert);
      setSuccess(neuerWert ? "Away-Status aktiviert" : "Away-Status deaktiviert");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function handleMessageSave(e: Event) {
    e.preventDefault();
    if (!away()) return;
    setBusy(true);
    setError("");
    setSuccess("");
    try {
      await setAway(true, awayMessage() || undefined);
      setSuccess("Away-Nachricht aktualisiert");
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section class={styles.section}>
      <div class={styles.sectionTitle}>Away-Status</div>
      <div class={styles.sectionBody}>
        <div class={styles.toggleRow}>
          <span class={styles.toggleLabel}>
            {away() ? "Away aktiv" : "Verfuegbar"}
          </span>
          <label class={styles.toggle}>
            <input
              type="checkbox"
              checked={away()}
              onChange={handleToggle}
              disabled={busy()}
            />
            <span class={styles.toggleSlider} />
          </label>
        </div>
        {away() && (
          <form class={styles.fieldRow} onSubmit={handleMessageSave}>
            <label class={styles.label}>Away-Nachricht (optional)</label>
            <input
              type="text"
              class={styles.input}
              value={awayMessage()}
              onInput={(e) => setAwayMessage(e.currentTarget.value)}
              disabled={busy()}
              maxlength={128}
              placeholder="Ich bin gerade nicht da..."
            />
            <div class={styles.btnRow}>
              <button type="submit" class={styles.btnPrimary} disabled={busy()}>
                {busy() ? "Wird gespeichert..." : "Nachricht speichern"}
              </button>
            </div>
          </form>
        )}
        {error() && <span class={styles.errorText}>{error()}</span>}
        {success() && <span class={styles.successText}>{success()}</span>}
      </div>
    </section>
  );
}

// --- Haupt-Seite ---

export default function AccountSettings() {
  return (
    <div class={styles.page}>
      <nav class={styles.breadcrumb}>
        <A href="/settings" class={styles.breadcrumbLink}>
          Einstellungen
        </A>
        <span class={styles.breadcrumbSep}>›</span>
        <span>Account</span>
      </nav>
      <div class={styles.titleRow}>
        <h1 class={styles.title}>Account-Verwaltung</h1>
      </div>
      <div class={styles.content}>
        <NicknameSection />
        <PasswordSection />
        <AwaySection />
      </div>
    </div>
  );
}
