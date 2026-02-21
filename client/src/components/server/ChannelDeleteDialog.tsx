import { createSignal } from "solid-js";
import { deleteChannel } from "../../bridge";
import Modal from "../ui/Modal";
import styles from "./ChannelDeleteDialog.module.css";

interface ChannelDeleteDialogProps {
  channelId: string;
  channelName: string;
  onClose: () => void;
  onDeleted: () => void;
}

export default function ChannelDeleteDialog(props: ChannelDeleteDialogProps) {
  const [error, setError] = createSignal<string | null>(null);
  const [busy, setBusy] = createSignal(false);

  const handleDelete = async () => {
    setBusy(true);
    setError(null);
    try {
      await deleteChannel(props.channelId);
      props.onDeleted();
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
        type="button"
        class={styles.btnDelete}
        onClick={handleDelete}
        disabled={busy()}
      >
        {busy() ? "Loesche..." : "Loeschen"}
      </button>
    </>
  );

  return (
    <Modal title="Channel loeschen" onClose={props.onClose} actions={actions}>
      <div class={styles.body}>
        <p class={styles.question}>
          Willst du den Channel <strong class={styles.channelName}>"{props.channelName}"</strong> wirklich loeschen?
        </p>
        <div class={styles.warning}>
          Alle Unterchannels werden ebenfalls geloescht. Diese Aktion kann nicht rueckgaengig gemacht werden.
        </div>
        {error() && <div class={styles.errorMsg}>{error()}</div>}
      </div>
    </Modal>
  );
}
