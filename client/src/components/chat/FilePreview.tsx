import { Show, createSignal } from "solid-js";
import type { FileInfo } from "../../bridge";
import { downloadFile } from "../../bridge";
import styles from "./FilePreview.module.css";

interface FilePreviewProps {
  fileInfo: FileInfo;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function isImage(mimeType: string): boolean {
  return mimeType.startsWith("image/");
}

function fileIcon(mimeType: string): string {
  if (mimeType.startsWith("image/")) return "🖼";
  if (mimeType.startsWith("video/")) return "🎬";
  if (mimeType.startsWith("audio/")) return "🎵";
  if (mimeType.includes("pdf")) return "📄";
  if (mimeType.includes("zip") || mimeType.includes("archive")) return "📦";
  if (mimeType.includes("text")) return "📝";
  return "📎";
}

export function FilePreview(props: FilePreviewProps) {
  const [downloading, setDownloading] = createSignal(false);

  const handleDownload = async () => {
    if (downloading()) return;
    setDownloading(true);
    try {
      const data = await downloadFile(props.fileInfo.id);
      const blob = new Blob([data.buffer as ArrayBuffer], { type: props.fileInfo.mime_type });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = props.fileInfo.filename;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) {
      console.error("Download fehlgeschlagen:", e);
    } finally {
      setDownloading(false);
    }
  };

  return (
    <div class={styles.filePreview}>
      <Show
        when={isImage(props.fileInfo.mime_type)}
        fallback={
          <button
            class={styles.fileDownload}
            onClick={handleDownload}
            disabled={downloading()}
          >
            <span class={styles.fileIcon}>{fileIcon(props.fileInfo.mime_type)}</span>
            <div class={styles.fileMeta}>
              <span class={styles.fileName}>{props.fileInfo.filename}</span>
              <span class={styles.fileSize}>{formatBytes(props.fileInfo.size_bytes)}</span>
            </div>
          </button>
        }
      >
        <div class={styles.imageContainer} onClick={handleDownload} title="Klicken zum Herunterladen">
          <img
            class={styles.imagePreview}
            src={`tauri://file/${props.fileInfo.id}`}
            alt={props.fileInfo.filename}
            loading="lazy"
          />
          <div class={styles.fileMeta} style={{ padding: "4px 8px 6px" }}>
            <span class={styles.fileSize}>{props.fileInfo.filename} · {formatBytes(props.fileInfo.size_bytes)}</span>
          </div>
        </div>
      </Show>
    </div>
  );
}
