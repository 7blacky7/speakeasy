import { createSignal, For, Show, onMount } from "solid-js";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Bookmark } from "../components/ui/MenuBar";
import { loadProfiles, type SoundProfile } from "../utils/soundProfiles";
import styles from "./BookmarkManager.module.css";

const BOOKMARKS_KEY = "speakeasy-bookmarks";

interface EditState {
  index: number;
  name: string;
  address: string;
  port: number;
  username: string;
  password: string;
  soundProfileId: string;
}

export default function BookmarkManager() {
  const [bookmarks, setBookmarks] = createSignal<Bookmark[]>(loadBookmarks());
  const [editing, setEditing] = createSignal<EditState | null>(null);
  const [soundProfiles, setSoundProfiles] = createSignal<SoundProfile[]>([]);

  onMount(() => {
    setSoundProfiles(loadProfiles());
  });

  function loadBookmarks(): Bookmark[] {
    try {
      const stored = localStorage.getItem(BOOKMARKS_KEY);
      return stored ? JSON.parse(stored) : [];
    } catch {
      return [];
    }
  }

  function persist(updated: Bookmark[]) {
    localStorage.setItem(BOOKMARKS_KEY, JSON.stringify(updated));
    setBookmarks(updated);
  }

  function handleDelete(index: number) {
    const updated = bookmarks().filter((_, i) => i !== index);
    persist(updated);
    if (editing()?.index === index) {
      setEditing(null);
    }
  }

  function startEdit(index: number) {
    const bm = bookmarks()[index];
    if (!bm) return;
    setEditing({
      index,
      name: bm.name,
      address: bm.address,
      port: bm.port,
      username: bm.username,
      password: bm.password ?? "",
      soundProfileId: bm.soundProfileId ?? "",
    });
  }

  function cancelEdit() {
    setEditing(null);
  }

  function saveEdit() {
    const edit = editing();
    if (!edit) return;
    const updated = [...bookmarks()];
    updated[edit.index] = {
      ...updated[edit.index],
      name: edit.name,
      address: edit.address,
      port: edit.port,
      username: edit.username,
      password: edit.password || undefined,
      soundProfileId: edit.soundProfileId || undefined,
    };
    persist(updated);
    setEditing(null);
  }

  function handleClose() {
    getCurrentWindow().close();
  }

  function profileName(id: string | undefined): string {
    if (!id) return "";
    return soundProfiles().find((p) => p.id === id)?.name ?? id;
  }

  return (
    <div class={styles.page}>
      <div class={styles.title}>Bookmarks verwalten</div>

      <Show
        when={bookmarks().length > 0}
        fallback={<div class={styles.empty}>Keine Bookmarks vorhanden</div>}
      >
        <table class={styles.table}>
          <thead>
            <tr>
              <th>Name</th>
              <th>Adresse</th>
              <th>Port</th>
              <th>Benutzer</th>
              <th>Passwort</th>
              <th>Sound-Profil</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            <For each={bookmarks()}>
              {(bm, i) => (
                <Show
                  when={editing()?.index !== i()}
                  fallback={
                    <tr>
                      <td>
                        <input
                          class={styles.editInput}
                          value={editing()!.name}
                          onInput={(e) =>
                            setEditing((prev) => prev ? { ...prev, name: e.currentTarget.value } : null)
                          }
                        />
                      </td>
                      <td>
                        <input
                          class={styles.editInput}
                          value={editing()!.address}
                          onInput={(e) =>
                            setEditing((prev) => prev ? { ...prev, address: e.currentTarget.value } : null)
                          }
                        />
                      </td>
                      <td>
                        <input
                          class={`${styles.editInput} ${styles.portInput}`}
                          type="number"
                          value={editing()!.port}
                          onInput={(e) =>
                            setEditing((prev) => prev ? { ...prev, port: Number(e.currentTarget.value) } : null)
                          }
                          min={1}
                          max={65535}
                        />
                      </td>
                      <td>
                        <input
                          class={styles.editInput}
                          value={editing()!.username}
                          onInput={(e) =>
                            setEditing((prev) => prev ? { ...prev, username: e.currentTarget.value } : null)
                          }
                        />
                      </td>
                      <td>
                        <input
                          class={styles.editInput}
                          type="password"
                          value={editing()!.password}
                          onInput={(e) =>
                            setEditing((prev) => prev ? { ...prev, password: e.currentTarget.value } : null)
                          }
                          placeholder="Leer = kein Passwort"
                        />
                      </td>
                      <td>
                        <select
                          class={styles.editInput}
                          value={editing()!.soundProfileId}
                          onChange={(e) =>
                            setEditing((prev) => prev ? { ...prev, soundProfileId: e.currentTarget.value } : null)
                          }
                        >
                          <option value="">-- Kein Profil --</option>
                          <For each={soundProfiles()}>
                            {(p) => <option value={p.id}>{p.name}</option>}
                          </For>
                        </select>
                      </td>
                      <td>
                        <div class={styles.actions}>
                          <button class={styles.btnSave} onClick={saveEdit}>
                            Speichern
                          </button>
                          <button class={styles.btnCancel} onClick={cancelEdit}>
                            Abbrechen
                          </button>
                        </div>
                      </td>
                    </tr>
                  }
                >
                  <tr>
                    <td>{bm.name}</td>
                    <td>{bm.address}</td>
                    <td>{bm.port}</td>
                    <td>{bm.username}</td>
                    <td class={styles.passwordCell}>
                      {bm.password ? "••••••" : ""}
                    </td>
                    <td>{profileName(bm.soundProfileId)}</td>
                    <td>
                      <div class={styles.actions}>
                        <button class={styles.btnEdit} onClick={() => startEdit(i())}>
                          Bearbeiten
                        </button>
                        <button class={styles.btnDelete} onClick={() => handleDelete(i())}>
                          Loeschen
                        </button>
                      </div>
                    </td>
                  </tr>
                </Show>
              )}
            </For>
          </tbody>
        </table>
      </Show>

      <div class={styles.footer}>
        <button class={styles.btnClose} onClick={handleClose}>
          Schliessen
        </button>
      </div>
    </div>
  );
}
