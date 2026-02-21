import { createSignal, Show } from "solid-js";
import { FileUploadButton } from "./FileUpload";
import styles from "./MessageInput.module.css";

interface MessageInputProps {
  channelId: string;
  channelName: string;
  onSend: (content: string) => Promise<void>;
  onFileUpload: (file: File) => Promise<void>;
  disabled?: boolean;
}

export function MessageInput(props: MessageInputProps) {
  const [text, setText] = createSignal("");
  const [sending, setSending] = createSignal(false);
  const [uploading, setUploading] = createSignal(false);

  let textareaRef: HTMLTextAreaElement | undefined;

  const handleInput = (e: Event) => {
    const ta = e.target as HTMLTextAreaElement;
    setText(ta.value);
    // Textarea-Hoehe automatisch anpassen
    ta.style.height = "auto";
    ta.style.height = Math.min(ta.scrollHeight, 180) + "px";
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleSend = async () => {
    const content = text().trim();
    if (!content || sending() || props.disabled) return;

    setSending(true);
    try {
      await props.onSend(content);
      setText("");
      if (textareaRef) {
        textareaRef.value = "";
        textareaRef.style.height = "auto";
      }
    } catch (e) {
      console.error("Senden fehlgeschlagen:", e);
    } finally {
      setSending(false);
    }
  };

  const handleFileSelected = async (file: File) => {
    if (uploading()) return;
    setUploading(true);
    try {
      await props.onFileUpload(file);
    } catch (e) {
      console.error("Upload fehlgeschlagen:", e);
    } finally {
      setUploading(false);
    }
  };

  return (
    <div class={styles.inputArea}>
      <Show when={uploading()}>
        <div class={styles.uploadProgress}>Datei wird hochgeladen...</div>
      </Show>
      <div class={styles.inputBox}>
        <FileUploadButton
          onFileSelected={handleFileSelected}
          uploading={uploading()}
        />
        <textarea
          ref={textareaRef}
          class={styles.textarea}
          placeholder={`Nachricht an #${props.channelName}`}
          rows={1}
          value={text()}
          onInput={handleInput}
          onKeyDown={handleKeyDown}
          disabled={sending() || props.disabled}
        />
        <button
          class={styles.sendBtn}
          onClick={handleSend}
          disabled={!text().trim() || sending() || props.disabled}
          title="Senden (Enter)"
        >
          ➤
        </button>
      </div>
    </div>
  );
}
