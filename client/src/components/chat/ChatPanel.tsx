import {
  createSignal,
  createResource,
  Show,
} from "solid-js";
import type { ChatMessage, ChannelInfo } from "../../bridge";
import {
  getMessageHistory,
  sendMessage,
  uploadFile,
} from "../../bridge";
import { MessageList } from "./MessageList";
import { MessageInput } from "./MessageInput";
import { DropOverlay } from "./FileUpload";
import styles from "./ChatPanel.module.css";

interface ChatPanelProps {
  channel: ChannelInfo | null;
}

export function ChatPanel(props: ChatPanelProps) {
  const [messages, setMessages] = createSignal<ChatMessage[]>([]);
  const [error, setError] = createSignal<string | null>(null);
  const [hasMore, setHasMore] = createSignal(false);
  const [dragOver, setDragOver] = createSignal(false);
  let dragCounter = 0;

  // History laden wenn sich der Kanal aendert
  const [_history] = createResource(
    () => props.channel?.id,
    async (channelId) => {
      if (!channelId) return;
      setError(null);
      try {
        const history = await getMessageHistory(channelId, undefined, 50);
        setMessages(history);
        setHasMore(history.length === 50);
      } catch (e) {
        setError("Nachrichten konnten nicht geladen werden.");
        console.error(e);
      }
    }
  );

  const handleSend = async (content: string) => {
    const ch = props.channel;
    if (!ch) return;
    setError(null);
    try {
      const msg = await sendMessage(ch.id, content);
      setMessages((prev) => [...prev, msg]);
    } catch (e) {
      setError("Nachricht konnte nicht gesendet werden.");
      console.error(e);
    }
  };

  const handleFileUpload = async (file: File) => {
    const ch = props.channel;
    if (!ch) return;
    setError(null);
    try {
      const msg = await uploadFile(ch.id, file);
      // uploadFile gibt eine ChatMessage zurueck
      setMessages((prev) => [...prev, msg as unknown as ChatMessage]);
    } catch (e) {
      setError("Datei konnte nicht hochgeladen werden.");
      console.error(e);
    }
  };

  const handleLoadMore = async () => {
    const ch = props.channel;
    if (!ch || messages().length === 0) return;
    const oldest = messages()[0];
    try {
      const older = await getMessageHistory(ch.id, oldest.created_at, 50);
      setMessages((prev) => [...older, ...prev]);
      setHasMore(older.length === 50);
    } catch (e) {
      setError("Aeltere Nachrichten konnten nicht geladen werden.");
    }
  };

  // Drag & Drop Handler
  const handleDragEnter = (e: DragEvent) => {
    e.preventDefault();
    dragCounter++;
    if (e.dataTransfer?.types.includes("Files")) {
      setDragOver(true);
    }
  };

  const handleDragLeave = (e: DragEvent) => {
    e.preventDefault();
    dragCounter--;
    if (dragCounter === 0) {
      setDragOver(false);
    }
  };

  const handleDragOver = (e: DragEvent) => {
    e.preventDefault();
  };

  const handleDrop = async (e: DragEvent) => {
    e.preventDefault();
    dragCounter = 0;
    setDragOver(false);
    const file = e.dataTransfer?.files[0];
    if (file && props.channel) {
      await handleFileUpload(file);
    }
  };

  return (
    <div
      class={styles.panel}
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      <DropOverlay visible={dragOver()} />

      <Show
        when={props.channel}
        fallback={
          <div class={styles.placeholder}>
            <span class={styles.placeholderIcon}>💬</span>
            <span class={styles.placeholderText}>Keinen Kanal ausgewaehlt</span>
            <span class={styles.placeholderSub}>
              Waehle einen Kanal aus der Liste links
            </span>
          </div>
        }
      >
        {(channel) => (
          <>
            <div class={styles.header}>
              <span class={styles.channelIcon}>#</span>
              <span class={styles.channelName}>{channel().name}</span>
              <Show when={channel().description}>
                <span class={styles.channelTopic}>{channel().description}</span>
              </Show>
            </div>

            <MessageList
              messages={messages()}
              onLoadMore={handleLoadMore}
              hasMore={hasMore()}
            />

            <Show when={error()}>
              <div class={styles.error}>{error()}</div>
            </Show>

            <MessageInput
              channelId={channel().id}
              channelName={channel().name}
              onSend={handleSend}
              onFileUpload={handleFileUpload}
            />
          </>
        )}
      </Show>
    </div>
  );
}
