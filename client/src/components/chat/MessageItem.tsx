import { Show } from "solid-js";
import type { ChatMessage } from "../../bridge";
import { FilePreview } from "./FilePreview";
import styles from "./MessageItem.module.css";

interface MessageItemProps {
  message: ChatMessage;
}

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  const heute = new Date();
  const istHeute =
    d.getDate() === heute.getDate() &&
    d.getMonth() === heute.getMonth() &&
    d.getFullYear() === heute.getFullYear();

  if (istHeute) {
    return `Heute um ${d.toLocaleTimeString("de-DE", { hour: "2-digit", minute: "2-digit" })}`;
  }
  return d.toLocaleString("de-DE", {
    day: "2-digit",
    month: "2-digit",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function MessageItem(props: MessageItemProps) {
  const msg = () => props.message;
  const initial = () => (msg().sender_name?.[0] ?? "?").toUpperCase();

  return (
    <Show
      when={msg().message_type !== "system"}
      fallback={
        <div class={styles.systemMessage}>{msg().content}</div>
      }
    >
      <div class={styles.message}>
        <div class={styles.avatar}>{initial()}</div>
        <div class={styles.body}>
          <div class={styles.header}>
            <span class={styles.senderName}>{msg().sender_name}</span>
            <span class={styles.timestamp}>{formatTimestamp(msg().created_at)}</span>
            <Show when={msg().edited_at}>
              <span class={styles.editedLabel}>(bearbeitet)</span>
            </Show>
          </div>
          <Show when={msg().reply_to}>
            <div class={styles.replyIndicator}>
              Antwort auf eine Nachricht
            </div>
          </Show>
          <Show
            when={msg().message_type === "file" && msg().file_info}
            fallback={<div class={styles.content}>{msg().content}</div>}
          >
            <FilePreview fileInfo={msg().file_info!} />
          </Show>
        </div>
      </div>
    </Show>
  );
}
