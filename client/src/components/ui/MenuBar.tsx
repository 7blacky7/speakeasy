import { createSignal, For, Show, onMount, onCleanup } from "solid-js";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { connectToServer } from "../../bridge";
import styles from "./MenuBar.module.css";

const BOOKMARKS_KEY = "speakeasy-bookmarks";

async function openSettingsWindow(route: string, title: string, width: number, height: number) {
  const label = route.replace(/\//g, "-").replace(/^-/, "");
  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await existing.setFocus();
    return;
  }
  const baseUrl = window.location.origin;
  new WebviewWindow(label, {
    url: `${baseUrl}${route}`,
    title,
    width,
    height,
    resizable: true,
    center: true,
  });
}

export interface Bookmark {
  name: string;
  address: string;
  port: number;
  username: string;
}

interface MenuBarProps {
  connected: boolean;
  serverName?: string;
  serverAddress?: string;
  serverPort?: number;
  username?: string;
  onConnect?: () => void;
  onDisconnect?: () => void;
  onBookmarkConnect?: (bm: Bookmark) => void;
  onBookmarkNewTab?: (bm: Bookmark) => void;
  onBookmarkEdit?: (bm: Bookmark, index: number) => void;
}

type OpenMenu = "server" | "bookmarks" | "settings" | null;

interface ContextMenuState {
  visible: boolean;
  x: number;
  y: number;
  bookmark: Bookmark | null;
  index: number;
}

export default function MenuBar(props: MenuBarProps) {
  const [openMenu, setOpenMenu] = createSignal<OpenMenu>(null);
  const [bookmarks, setBookmarks] = createSignal<Bookmark[]>([]);
  const [bookmarkSaved, setBookmarkSaved] = createSignal(false);
  const [ctxMenu, setCtxMenu] = createSignal<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    bookmark: null,
    index: -1,
  });
  let menubarRef: HTMLDivElement | undefined;
  let ctxMenuRef: HTMLDivElement | undefined;

  const loadBookmarks = () => {
    try {
      const stored = localStorage.getItem(BOOKMARKS_KEY);
      if (stored) {
        setBookmarks(JSON.parse(stored));
      }
    } catch {
      setBookmarks([]);
    }
  };

  const saveBookmark = () => {
    const address = props.serverAddress || localStorage.getItem("speakeasy_last_address") || "";
    const port = props.serverPort || Number(localStorage.getItem("speakeasy_last_port") || "9001");
    const username = props.username || localStorage.getItem("speakeasy_last_username") || "";
    const name = props.serverName || `${address}:${port}`;

    if (!address) return;

    const current = [...bookmarks()];
    const exists = current.some((b) => b.address === address && b.port === port);
    if (!exists) {
      current.push({ name, address, port, username });
      localStorage.setItem(BOOKMARKS_KEY, JSON.stringify(current));
      setBookmarks(current);
    }
    setBookmarkSaved(true);
    setTimeout(() => setBookmarkSaved(false), 2000);
  };

  const handleBookmarkConnect = async (bm: Bookmark) => {
    if (props.onBookmarkConnect) {
      props.onBookmarkConnect(bm);
    } else {
      try {
        await connectToServer({
          address: bm.address,
          port: bm.port,
          username: bm.username,
        });
      } catch (e) {
        console.error("Bookmark-Verbindung fehlgeschlagen:", e);
      }
    }
  };

  const handleBookmarkRightClick = (e: MouseEvent, bm: Bookmark, index: number) => {
    e.preventDefault();
    e.stopPropagation();
    setCtxMenu({
      visible: true,
      x: e.clientX,
      y: e.clientY,
      bookmark: bm,
      index,
    });
  };

  const closeCtxMenu = () => {
    setCtxMenu({ visible: false, x: 0, y: 0, bookmark: null, index: -1 });
  };

  const ctxConnect = () => {
    const bm = ctxMenu().bookmark;
    closeCtxMenu();
    setOpenMenu(null);
    if (bm) handleBookmarkConnect(bm);
  };

  const ctxNewTab = () => {
    const bm = ctxMenu().bookmark;
    closeCtxMenu();
    setOpenMenu(null);
    if (bm && props.onBookmarkNewTab) {
      props.onBookmarkNewTab(bm);
    } else if (bm) {
      handleBookmarkConnect(bm);
    }
  };

  const ctxEdit = () => {
    const ctx = ctxMenu();
    closeCtxMenu();
    setOpenMenu(null);
    if (ctx.bookmark && props.onBookmarkEdit) {
      props.onBookmarkEdit(ctx.bookmark, ctx.index);
    }
  };

  const handleOutsideClick = (e: MouseEvent) => {
    if (ctxMenu().visible && ctxMenuRef && !ctxMenuRef.contains(e.target as Node)) {
      closeCtxMenu();
    }
    if (menubarRef && !menubarRef.contains(e.target as Node)) {
      setOpenMenu(null);
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      closeCtxMenu();
      setOpenMenu(null);
    }
  };

  onMount(() => {
    loadBookmarks();
    document.addEventListener("mousedown", handleOutsideClick);
    document.addEventListener("keydown", handleKeyDown);
  });

  onCleanup(() => {
    document.removeEventListener("mousedown", handleOutsideClick);
    document.removeEventListener("keydown", handleKeyDown);
  });

  const toggleMenu = (menu: OpenMenu) => {
    closeCtxMenu();
    if (openMenu() === menu) {
      setOpenMenu(null);
    } else {
      if (menu === "bookmarks") loadBookmarks();
      setOpenMenu(menu);
    }
  };

  const closeAndAction = (action: () => void) => {
    setOpenMenu(null);
    action();
  };

  return (
    <div class={styles.menubar} ref={menubarRef}>
      {/* Server-Menue */}
      <div class={styles.menuItem}>
        <button
          class={`${styles.menuBtn} ${openMenu() === "server" ? styles.open : ""}`}
          onClick={() => toggleMenu("server")}
        >
          Server
        </button>
        <Show when={openMenu() === "server"}>
          <div class={styles.dropdown}>
            <button
              class={styles.dropdownItem}
              onClick={() => closeAndAction(() => props.onConnect?.())}
            >
              <span class={styles.dropdownLabel}>Verbinden...</span>
            </button>
            <button
              class={`${styles.dropdownItem} ${!props.connected ? styles.disabled : ""}`}
              onClick={() => closeAndAction(() => props.onDisconnect?.())}
            >
              <span class={styles.dropdownLabel}>Trennen</span>
            </button>
            <div class={styles.separator} />
            <button
              class={styles.dropdownItem}
              onClick={() => closeAndAction(() => window.close())}
            >
              <span class={styles.dropdownLabel}>Beenden</span>
            </button>
          </div>
        </Show>
      </div>

      {/* Bookmarks-Menue */}
      <div class={styles.menuItem}>
        <button
          class={`${styles.menuBtn} ${openMenu() === "bookmarks" ? styles.open : ""}`}
          onClick={() => toggleMenu("bookmarks")}
        >
          Bookmarks
        </button>
        <Show when={openMenu() === "bookmarks"}>
          <div class={styles.dropdown}>
            <button
              class={`${styles.dropdownItem} ${!props.connected ? styles.disabled : ""}`}
              onClick={() => {
                if (props.connected) {
                  saveBookmark();
                  setOpenMenu(null);
                }
              }}
            >
              <span class={styles.dropdownLabel}>
                {bookmarkSaved() ? "Gespeichert!" : "Bookmark hinzufuegen"}
              </span>
            </button>
            <Show when={bookmarks().length > 0}>
              <div class={styles.separator} />
              <For each={bookmarks()}>
                {(bm, i) => (
                  <button
                    class={styles.bookmarkEntry}
                    onClick={() =>
                      closeAndAction(() => handleBookmarkConnect(bm))
                    }
                    onContextMenu={(e) => handleBookmarkRightClick(e, bm, i())}
                  >
                    <span class={styles.dropdownLabel}>{bm.name}</span>
                    <span class={styles.bookmarkAddress}>
                      {bm.address}:{bm.port}
                    </span>
                  </button>
                )}
              </For>
            </Show>
            <div class={styles.separator} />
            <button
              class={styles.dropdownItem}
              onClick={() => closeAndAction(() => openSettingsWindow("/bookmarks", "Bookmarks verwalten", 600, 400))}
            >
              <span class={styles.dropdownLabel}>Alle anzeigen...</span>
            </button>
          </div>
        </Show>
      </div>

      {/* Einstellungen-Menue */}
      <div class={styles.menuItem}>
        <button
          class={`${styles.menuBtn} ${openMenu() === "settings" ? styles.open : ""}`}
          onClick={() => toggleMenu("settings")}
        >
          Einstellungen
        </button>
        <Show when={openMenu() === "settings"}>
          <div class={styles.dropdown}>
            <button
              class={styles.dropdownItem}
              onClick={() => closeAndAction(() => openSettingsWindow("/settings/audio", "Audio-Einstellungen", 750, 650))}
            >
              <span class={styles.dropdownLabel}>Sound</span>
            </button>
            <button
              class={styles.dropdownItem}
              onClick={() => closeAndAction(() => openSettingsWindow("/settings/plugins", "Plugins", 700, 500))}
            >
              <span class={styles.dropdownLabel}>Plugins</span>
            </button>
            <button
              class={styles.dropdownItem}
              onClick={() => closeAndAction(() => openSettingsWindow("/settings/account", "Account", 550, 450))}
            >
              <span class={styles.dropdownLabel}>Account</span>
            </button>
          </div>
        </Show>
      </div>

      {/* Bookmark-Kontextmenu */}
      <Show when={ctxMenu().visible}>
        <div
          ref={ctxMenuRef}
          class={styles.contextMenu}
          style={{ left: `${ctxMenu().x}px`, top: `${ctxMenu().y}px` }}
        >
          <button class={styles.contextMenuItem} onClick={ctxConnect}>
            Verbinden
          </button>
          <button class={styles.contextMenuItem} onClick={ctxNewTab}>
            In neuem Tab verbinden
          </button>
          <div class={styles.separator} />
          <button class={styles.contextMenuItem} onClick={ctxEdit}>
            Bearbeiten
          </button>
        </div>
      </Show>
    </div>
  );
}
