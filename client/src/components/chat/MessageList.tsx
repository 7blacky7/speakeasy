import { For, Show, onMount, createEffect } from "solid-js";
import type { ChatMessage } from "../../bridge";
import { MessageItem } from "./MessageItem";
import styles from "./MessageList.module.css";

interface MessageListProps {
  messages: ChatMessage[];
  onLoadMore?: () => void;
  hasMore?: boolean;
  loading?: boolean;
}

export function MessageList(props: MessageListProps) {
  let listRef: HTMLDivElement | undefined;

  // Automatisch nach unten scrollen bei neuen Nachrichten
  createEffect(() => {
    const msgs = props.messages;
    if (msgs.length > 0 && listRef) {
      const isAtBottom =
        listRef.scrollHeight - listRef.scrollTop - listRef.clientHeight < 100;
      if (isAtBottom) {
        listRef.scrollTop = listRef.scrollHeight;
      }
    }
  });

  // Beim ersten Laden ganz nach unten scrollen
  onMount(() => {
    if (listRef) {
      listRef.scrollTop = listRef.scrollHeight;
    }
  });

  return (
    <div class={styles.list} ref={listRef}>
      <Show when={props.hasMore && !props.loading}>
        <div class={styles.loadMore}>
          <button class={styles.loadMoreBtn} onClick={props.onLoadMore}>
            Aeltere Nachrichten laden
          </button>
        </div>
      </Show>

      <Show
        when={props.messages.length > 0}
        fallback={
          <div class={styles.emptyState}>
            <span class={styles.emptyIcon}>💬</span>
            <span>Noch keine Nachrichten. Schreib die erste!</span>
          </div>
        }
      >
        <For each={props.messages}>
          {(msg) => <MessageItem message={msg} />}
        </For>
      </Show>
    </div>
  );
}
