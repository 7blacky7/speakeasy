import { createSignal, Show } from "solid-js";
import type { ChannelInfo } from "../../bridge";
import { createChannel } from "../../bridge";
import Modal from "../ui/Modal";
import CustomSelect from "../ui/CustomSelect";
import styles from "./ChannelCreateDialog.module.css";

interface ChannelCreateDialogProps {
  channels: ChannelInfo[];
  defaultParentId?: string | null;
  onClose: () => void;
  onCreated: () => void;
}

function deriveDefaultType(
  channels: ChannelInfo[]
): "permanent" | "semi_permanent" | "temporary" {
  if (channels.length === 0) return "permanent";
  const permanentCount = channels.filter(
    (c) => (c as ChannelInfo & { channel_type?: string }).channel_type === "permanent"
  ).length;
  return permanentCount >= channels.length / 2 ? "permanent" : "temporary";
}

function suggestChannelName(channels: ChannelInfo[]): string {
  const existing = new Set(channels.map((c) => c.name));
  let name = "Neuer Channel";
  if (!existing.has(name)) return name;
  let i = 2;
  while (existing.has(name + " " + i)) {
    i++;
  }
  return name + " " + i;
}

export default function ChannelCreateDialog(props: ChannelCreateDialogProps) {
  const [name, setName] = createSignal(suggestChannelName(props.channels));
  const [description, setDescription] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [maxClients, setMaxClients] = createSignal(0);
  const [parentId, setParentId] = createSignal<string>(
    props.defaultParentId ?? ""
  );
  const [channelType, setChannelType] = createSignal<
    "permanent" | "semi_permanent" | "temporary"
  >(deriveDefaultType(props.channels));
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [expanded, setExpanded] = createSignal(false);

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    if (!name().trim()) {
      setError("Name ist erforderlich.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await createChannel(
        name().trim(),
        description().trim() || undefined,
        password().trim() || undefined,
        maxClients() > 0 ? maxClients() : undefined,
        parentId() || undefined
      );
      props.onCreated();
      props.onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
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
        form="channel-create-form"
        class={styles.btnPrimary}
        disabled={busy() || !name().trim()}
      >
        {busy() ? "Erstelle..." : "Erstellen"}
      </button>
    </>
  );

  return (
    <Modal title="Channel erstellen" onClose={props.onClose} actions={actions}>
      <form id="channel-create-form" onSubmit={handleSubmit} class={styles.form}>
        {/* Name */}
        <div class={styles.field}>
          <label class={styles.label} for="ch-name">
            Name <span class={styles.required}>*</span>
          </label>
          <input
            id="ch-name"
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

        {/* Erweiterter Bereich */}
        <Show when={expanded()}>
          <div class={styles.expandedSection}>
            {/* Beschreibung */}
            <div class={styles.field}>
              <label class={styles.label} for="ch-desc">
                Beschreibung
              </label>
              <textarea
                id="ch-desc"
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
              <label class={styles.label} for="ch-pw">
                Passwort
              </label>
              <input
                id="ch-pw"
                type="password"
                class={styles.input}
                value={password()}
                onInput={(e) => setPassword(e.currentTarget.value)}
                placeholder="Leer lassen fuer keinen Schutz"
              />
            </div>

            {/* Max Clients */}
            <div class={styles.field}>
              <label class={styles.label} for="ch-max">
                Max. Clients <span class={styles.hint}>(0 = unbegrenzt)</span>
              </label>
              <input
                id="ch-max"
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

            {/* Parent-Channel */}
            <div class={styles.field}>
              <label class={styles.label} for="ch-parent">
                Parent-Channel
              </label>
              <CustomSelect
                value={parentId()}
                options={[
                  { value: "", label: "Kein Parent (Root)" },
                  ...props.channels.map(ch => ({ value: ch.id, label: ch.name }))
                ]}
                onChange={setParentId}
                ariaLabel="Parent-Channel"
              />
            </div>

            {/* Channel-Typ */}
            <div class={styles.field}>
              <span class={styles.label}>Channel-Typ</span>
              <div class={styles.radioGroup}>
                <label class={styles.radioLabel}>
                  <input
                    type="radio"
                    name="channel-type"
                    value="permanent"
                    checked={channelType() === "permanent"}
                    onChange={() => setChannelType("permanent")}
                  />
                  Permanent
                </label>
                <label class={styles.radioLabel}>
                  <input
                    type="radio"
                    name="channel-type"
                    value="semi_permanent"
                    checked={channelType() === "semi_permanent"}
                    onChange={() => setChannelType("semi_permanent")}
                  />
                  Semi-Permanent
                </label>
                <label class={styles.radioLabel}>
                  <input
                    type="radio"
                    name="channel-type"
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
