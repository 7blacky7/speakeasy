import { Component, createSignal, createResource, For, Show } from "solid-js";
import {
  listPlugins,
  enablePlugin,
  disablePlugin,
  unloadPlugin,
  type PluginInfo,
  type PluginInstallResult,
} from "../../bridge";
import PluginCard from "./PluginCard";
import PluginInstall from "./PluginInstall";
import styles from "./PluginManager.module.css";

const PluginManager: Component = () => {
  const [showInstall, setShowInstall] = createSignal(false);
  const [actionError, setActionError] = createSignal<string | null>(null);

  const [plugins, { refetch }] = createResource<PluginInfo[]>(
    () => listPlugins().catch(() => [])
  );

  async function handleEnable(id: string) {
    setActionError(null);
    try {
      await enablePlugin(id);
      refetch();
    } catch (e) {
      setActionError(`Aktivierung fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  async function handleDisable(id: string) {
    setActionError(null);
    try {
      await disablePlugin(id);
      refetch();
    } catch (e) {
      setActionError(`Deaktivierung fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  async function handleUnload(id: string) {
    setActionError(null);
    try {
      await unloadPlugin(id);
      refetch();
    } catch (e) {
      setActionError(`Entfernen fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`);
    }
  }

  function handleInstalled(_result: PluginInstallResult) {
    setShowInstall(false);
    refetch();
  }

  return (
    <div class={styles.container}>
      <div class={styles.toolbar}>
        <h2 class={styles.heading}>Plugins</h2>
        <button
          class={styles.btnInstall}
          onClick={() => setShowInstall((v) => !v)}
        >
          {showInstall() ? "Abbrechen" : "Plugin installieren"}
        </button>
      </div>

      <Show when={showInstall()}>
        <PluginInstall onInstalled={handleInstalled} />
      </Show>

      <Show when={actionError()}>
        <div class={styles.error}>{actionError()}</div>
      </Show>

      <Show when={plugins.loading}>
        <div class={styles.loading}>Lade Plugins...</div>
      </Show>

      <Show when={!plugins.loading && plugins()?.length === 0}>
        <div class={styles.empty}>
          Keine Plugins installiert. Klicke auf "Plugin installieren" um zu beginnen.
        </div>
      </Show>

      <Show when={plugins()}>
        <div class={styles.list}>
          <For each={plugins()}>
            {(plugin) => (
              <PluginCard
                plugin={plugin}
                onEnable={handleEnable}
                onDisable={handleDisable}
                onUnload={handleUnload}
              />
            )}
          </For>
        </div>
      </Show>
    </div>
  );
};

export default PluginManager;
