import { createSignal, Show } from "solid-js";
import type { ChannelInfo } from "../../bridge";
import { editChannel } from "../../bridge";
import Modal from "../ui/Modal";
import styles from "./ChannelCreateDialog.module.css";

interface ChannelEditDialogProps {
  channel: ChannelInfo & { channel_type?: "permanent" | "semi_permanent" | "temporary" };
  channels: ChannelInfo[];
  onClose: () => void;
  onEdited: () => void;
}

export default function ChannelEditDialog(props: ChannelEditDialogProps) {
  const [name, setName] = createSignal(props.channel.name);
  const [description, setDescription] = createSignal(props.channel.description ?? "");
  const [password, setPassword] = createSignal("");
  const [maxClients, setMaxClients] = createSignal(props.channel.max_clients ?? 0);
  const [channelType, setChannelType] = createSignal<
    "permanent" | "semi_permanent" | "temporary"
  >(props.channel.channel_type ?? "permanent");
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [expanded, setExpanded] = createSignal(true);

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    if (!name().trim()) {
      setError("Name ist erforderlich.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await editChannel(
        props.channel.id,
        name().trim(),
        description().trim() || undefined,
        password().trim() || undefined,
        maxClients() > 0 ? maxClients() : undefined
      );
      props.onEdited();
      props.onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const parentName = () => {
    if (!props.channel.parent_id) return "Kein Parent (Root)";
    const p = props.channels.find((ch) => ch.id === props.channel.parent_id);
    return p ? p.name : "Unbekannt";
  };

  const actions = (
    <>
      <button
        type="button"
        class={styles.btnCancel}
        onClick={props.onClose}
        disabled={busy()}
      >
        Abbrechen
      </button>
      <button
        type="submit"
        form="channel-edit-form"
        class={styles.btnPrimary}
        disabled={busy() || !name().trim()}
      >
        {busy() ? "Speichere..." : "Speichern"}
      </button>
    </>
  );

  return (
    <Modal title="Channel bearbeiten" onClose={props.onClose} actions={actions}>
      <form id="channel-edit-form" onSubmit={handleSubmit} class={styles.form}>
        {/* Name */}
        <div class={styles.field}>
          <label class={styles.label} for="ch-edit-name">
            Name <span class={styles.required}>*</span>
          </label>
          <input
            id="ch-edit-name"
            type="text"
            class={styles.input}
            value={name()}
            onInput={(e) => setName(e.currentTarget.value)}
            placeholder="Channel-Name"
            maxLength={64}
            autofocus
          />
        </div>

        {/* Erweiterte Einstellungen Toggle */}
        <button
          type="button"
          class={styles.expandToggle}
          onClick={() => setExpanded((v) => !v)}
        >
          <span class={styles.expandArrow}>{expanded() ? "▲" : "▼"}</span>
          Erweiterte Einstellungen
        </button>

        {/* Erweiterter Bereich - beim Bearbeiten standardmaessig aufgeklappt */}
        <Show when={expanded()}>
          <div class={styles.expandedSection}>
            {/* Beschreibung */}
            <div class={styles.field}>
              <label class={styles.label} for="ch-edit-desc">
                Beschreibung
              </label>
              <textarea
                id="ch-edit-desc"
                class={styles.textarea}
                value={description()}
                onInput={(e) => setDescription(e.currentTarget.value)}
                placeholder="Optionale Beschreibung"
                rows={3}
                maxLength={512}
              />
            </div>

            {/* Passwort */}
            <div class={styles.field}>
              <label class={styles.label} for="ch-edit-pw">
                Neues Passwort
              </label>
              <input
                id="ch-edit-pw"
                type="password"
                class={styles.input}
                value={password()}
                onInput={(e) => setPassword(e.currentTarget.value)}
                placeholder="Leer lassen um unveraendert zu lassen"
              />
            </div>

            {/* Max Clients */}
            <div class={styles.field}>
              <label class={styles.label} for="ch-edit-max">
                Max. Clients <span class={styles.hint}>(0 = unbegrenzt)</span>
              </label>
              <input
                id="ch-edit-max"
                type="number"
                class={styles.input}
                value={maxClients()}
                onInput={(e) =>
                  setMaxClients(Math.max(0, parseInt(e.currentTarget.value) || 0))
                }
                min={0}
                max={512}
              />
            </div>

            {/* Parent-Channel (nur Info, nicht aenderbar) */}
            <div class={styles.field}>
              <span class={styles.label}>Parent-Channel</span>
              <span class={styles.infoText}>{parentName()}</span>
            </div>

            {/* Channel-Typ */}
            <div class={styles.field}>
              <span class={styles.label}>Channel-Typ</span>
              <div class={styles.radioGroup}>
                <label class={styles.radioLabel}>
                  <input
                    type="radio"
                    name="ch-edit-type"
                    value="permanent"
                    checked={channelType() === "permanent"}
                    onChange={() => setChannelType("permanent")}
                  />
                  Permanent
                </label>
                <label class={styles.radioLabel}>
                  <input
                    type="radio"
                    name="ch-edit-type"
                    value="semi_permanent"
                    checked={channelType() === "semi_permanent"}
                    onChange={() => setChannelType("semi_permanent")}
                  />
                  Semi-Permanent
                </label>
                <label class={styles.radioLabel}>
                  <input
                    type="radio"
                    name="ch-edit-type"
                    value="temporary"
                    checked={channelType() === "temporary"}
                    onChange={() => setChannelType("temporary")}
                  />
                  Temporaer
                </label>
              </div>
            </div>
          </div>
        </Show>

        {/* Fehler */}
        {error() && <div class={styles.errorMsg}>{error()}</div>}
      </form>
    </Modal>
  );
}
