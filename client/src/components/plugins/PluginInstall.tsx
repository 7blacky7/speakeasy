import { Component, createSignal } from "solid-js";
import { installPlugin, type PluginInstallResult } from "../../bridge";
import styles from "./PluginInstall.module.css";

interface PluginInstallProps {
  onInstalled: (result: PluginInstallResult) => void;
}

const PluginInstall: Component<PluginInstallProps> = (props) => {
  const [path, setPath] = createSignal("");
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal<string | null>(null);
  const [warning, setWarning] = createSignal<string | null>(null);

  async function handleInstall() {
    const p = path().trim();
    if (!p) {
      setError("Bitte einen Pfad eingeben.");
      return;
    }
    setError(null);
    setWarning(null);
    setLoading(true);
    try {
      const result = await installPlugin(p);
      if (result.trust_level === "NichtSigniert") {
        setWarning(
          `Plugin "${result.name}" ist nicht signiert. Nur aus vertrauenswuerdigen Quellen laden.`
        );
      }
      props.onInstalled(result);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div class={styles.container}>
      <h3 class={styles.title}>Plugin installieren</h3>
      <p class={styles.hint}>
        Pfad zum Plugin-Verzeichnis eingeben (muss eine{" "}
        <code>manifest.toml</code> enthalten).
      </p>

      {warning() && <div class={styles.warning}>{warning()}</div>}
      {error() && <div class={styles.error}>{error()}</div>}

      <div class={styles.row}>
        <input
          class={styles.input}
          type="text"
          placeholder="/pfad/zum/plugin"
          value={path()}
          onInput={(e) => setPath(e.currentTarget.value)}
          disabled={loading()}
        />
        <button
          class={styles.btn}
          onClick={handleInstall}
          disabled={loading() || path().trim() === ""}
        >
          {loading() ? "Installiere..." : "Installieren"}
        </button>
      </div>
    </div>
  );
};

export default PluginInstall;
