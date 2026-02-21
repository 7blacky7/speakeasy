import { Component, Show } from "solid-js";
import type { PluginInfo } from "../../bridge";
import styles from "./PluginCard.module.css";

interface PluginCardProps {
  plugin: PluginInfo;
  onEnable: (id: string) => void;
  onDisable: (id: string) => void;
  onUnload: (id: string) => void;
}

function trustLabel(trust: PluginInfo["trust_level"]): string {
  switch (trust) {
    case "Vertrauenswuerdig": return "Vertrauenswuerdig";
    case "Signiert": return "Signiert";
    case "NichtSigniert": return "Nicht signiert";
  }
}

function trustClass(trust: PluginInfo["trust_level"]): string {
  switch (trust) {
    case "Vertrauenswuerdig": return styles.trustOk;
    case "Signiert": return styles.trustOk;
    case "NichtSigniert": return styles.trustWarn;
  }
}

function stateLabel(state: PluginInfo["state"]): string {
  if (state === "Aktiv") return "Aktiv";
  if (state === "Geladen") return "Geladen";
  if (state === "Deaktiviert") return "Deaktiviert";
  if (typeof state === "object" && "Fehler" in state) return `Fehler: ${state.Fehler}`;
  return "Unbekannt";
}

const PluginCard: Component<PluginCardProps> = (props) => {
  const isActive = () => props.plugin.state === "Aktiv";
  const hasError = () =>
    typeof props.plugin.state === "object" && "Fehler" in props.plugin.state;

  return (
    <div class={`${styles.card} ${hasError() ? styles.cardError : ""}`}>
      <div class={styles.header}>
        <div class={styles.meta}>
          <span class={styles.name}>{props.plugin.name}</span>
          <span class={styles.version}>v{props.plugin.version}</span>
          <span class={`${styles.trust} ${trustClass(props.plugin.trust_level)}`}>
            {trustLabel(props.plugin.trust_level)}
          </span>
        </div>
        <div class={styles.state}>{stateLabel(props.plugin.state)}</div>
      </div>

      <Show when={props.plugin.description}>
        <p class={styles.description}>{props.plugin.description}</p>
      </Show>

      <div class={styles.author}>Autor: {props.plugin.author}</div>

      <Show when={props.plugin.trust_level === "NichtSigniert"}>
        <div class={styles.warning}>
          Dieses Plugin ist nicht signiert. Nur vertrauenswuerdige Quellen laden.
        </div>
      </Show>

      <div class={styles.actions}>
        <Show when={!isActive()}>
          <button
            class={styles.btnEnable}
            onClick={() => props.onEnable(props.plugin.id)}
            disabled={hasError()}
          >
            Aktivieren
          </button>
        </Show>
        <Show when={isActive()}>
          <button
            class={styles.btnDisable}
            onClick={() => props.onDisable(props.plugin.id)}
          >
            Deaktivieren
          </button>
        </Show>
        <button
          class={styles.btnUnload}
          onClick={() => props.onUnload(props.plugin.id)}
        >
          Entfernen
        </button>
      </div>
    </div>
  );
};

export default PluginCard;
