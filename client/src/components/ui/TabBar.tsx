import { For, Show } from "solid-js";
import { ContextMenu, createContextMenu, type ContextMenuItem } from "./ContextMenu";
import styles from "./TabBar.module.css";

export interface ServerTab {
  id: string;
  name: string;
  active: boolean;
}

interface TabBarProps {
  tabs: ServerTab[];
  onTabSelect: (tabId: string) => void;
  onTabClose: (tabId: string) => void;
  onNewTab: () => void;
}

export default function TabBar(props: TabBarProps) {
  const { menuState, show: showMenu, hide: hideMenu } = createContextMenu();

  const handleContextMenu = (e: MouseEvent, tab: ServerTab) => {
    const items: ContextMenuItem[] = [
      {
        id: "close",
        label: "Tab schliessen",
        icon: "\u2715",
        onClick: () => props.onTabClose(tab.id),
      },
    ];
    showMenu(e, items);
  };

  return (
    <div class={styles.tabbar}>
      <For each={props.tabs}>
        {(tab) => (
          <button
            class={`${styles.tab} ${tab.active ? styles.active : ""}`}
            onClick={() => props.onTabSelect(tab.id)}
            onContextMenu={(e) => handleContextMenu(e, tab)}
          >
            <span class={styles.tabName}>{tab.name}</span>
            <span
              class={styles.tabClose}
              onClick={(e) => {
                e.stopPropagation();
                props.onTabClose(tab.id);
              }}
            >
              {"\u2715"}
            </span>
          </button>
        )}
      </For>

      <button class={styles.addTab} onClick={props.onNewTab} title="Neue Verbindung">
        +
      </button>

      <Show when={menuState().visible}>
        <ContextMenu
          items={menuState().items}
          x={menuState().x}
          y={menuState().y}
          onClose={hideMenu}
        />
      </Show>
    </div>
  );
}
