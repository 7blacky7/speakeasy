import { Show } from "solid-js";
import styles from "./FileUpload.module.css";

interface FileUploadProps {
  onFileSelected: (file: File) => void;
  uploading?: boolean;
  dragOver?: boolean;
}

export function FileUploadButton(props: FileUploadProps) {
  let inputRef: HTMLInputElement | undefined;

  const handleClick = () => {
    inputRef?.click();
  };

  const handleChange = (e: Event) => {
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    if (file) {
      props.onFileSelected(file);
      input.value = "";
    }
  };

  return (
    <>
      <button
        class={styles.uploadBtn}
        onClick={handleClick}
        disabled={props.uploading}
        title="Datei hochladen"
      >
        📎
      </button>
      <input
        ref={inputRef}
        type="file"
        class={styles.hiddenInput}
        onChange={handleChange}
      />
    </>
  );
}

interface DropOverlayProps {
  visible: boolean;
}

export function DropOverlay(props: DropOverlayProps) {
  return (
    <Show when={props.visible}>
      <div class={styles.dropOverlay}>
        <div class={styles.dropOverlayContent}>
          <span class={styles.dropIcon}>📂</span>
          <span class={styles.dropText}>Datei hier ablegen</span>
        </div>
      </div>
    </Show>
  );
}
