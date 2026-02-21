import { createSignal, For, Show, onMount, onCleanup } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import styles from "./MenuBar.module.css";

async function openSettingsWindow(route: string, title: string, width: number, height: number) {
  const label = route.replace(/\//g, "-").replace(/^-/, "");
  const existing = await WebviewWindow.getByLabel(label);
  if (existing) {
    await existing.setFocus();
    return;
  }
  new WebviewWindow(label, {
    url: route,
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
  onConnect?: () => void;
  onDisconnect?: () => void;
  onBookmarkAdd?: () => void;
}

type OpenMenu = "server" | "bookmarks" | "settings" | null;

export default function MenuBar(props: MenuBarProps) {
  const [openMenu, setOpenMenu] = createSignal<OpenMenu>(null);
  const [bookmarks, setBookmarks] = createSignal<Bookmark[]>([]);
  const navigate = useNavigate();
  let menubarRef: HTMLDivElement | undefined;

  const loadBookmarks = () => {
    try {
      const stored = localStorage.getItem("speakeasy-bookmarks");
      if (stored) {
        setBookmarks(JSON.parse(stored));
      }
    } catch {
      setBookmarks([]);
    }
  };

  const handleOutsideClick = (e: MouseEvent) => {
    if (menubarRef && !menubarRef.contains(e.target as Node)) {
      setOpenMenu(null);
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
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
              onClick={() => closeAndAction(() => props.onBookmarkAdd?.())}
            >
              <span class={styles.dropdownLabel}>Bookmark hinzufuegen</span>
            </button>
            <Show when={bookmarks().length > 0}>
              <div class={styles.separator} />
              <For each={bookmarks()}>
                {(bm) => (
                  <button
                    class={styles.bookmarkEntry}
                    onClick={() =>
                      closeAndAction(() => navigate(`/server/${encodeURIComponent(bm.address)}:${bm.port}`))
                    }
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
              onClick={() => closeAndAction(() => navigate("/"))}
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
    </div>
  );
}
