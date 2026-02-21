import { createSignal, For, Show } from "solid-js";
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
  onTabsReorder?: (tabs: ServerTab[]) => void;
  onCloseOtherTabs?: (keepTabId: string) => void;
}

export default function TabBar(props: TabBarProps) {
  const { menuState, show: showMenu, hide: hideMenu } = createContextMenu();
  const [dragOverId, setDragOverId] = createSignal<string | null>(null);
  let dragSourceId: string | null = null;

  const handleContextMenu = (e: MouseEvent, tab: ServerTab) => {
    const items: ContextMenuItem[] = [
      {
        id: "close",
        label: "Tab schliessen",
        icon: "\u2715",
        onClick: () => props.onTabClose(tab.id),
      },
      {
        id: "close-others",
        label: "Andere Tabs schliessen",
        disabled: props.tabs.length <= 1,
        onClick: () => {
          if (props.onCloseOtherTabs) {
            props.onCloseOtherTabs(tab.id);
          }
        },
      },
    ];
    showMenu(e, items);
  };

  const handleMiddleClick = (e: MouseEvent, tab: ServerTab) => {
    if (e.button !== 1) return;
    e.preventDefault();
    e.stopPropagation();
    props.onTabClose(tab.id);
  };

  // --- Drag & Drop ---
  const handleDragStart = (e: DragEvent, tab: ServerTab) => {
    dragSourceId = tab.id;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", tab.id);
    }
  };

  const handleDragOver = (e: DragEvent, tab: ServerTab) => {
    e.preventDefault();
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = "move";
    }
    setDragOverId(tab.id);
  };

  const handleDragLeave = () => {
    setDragOverId(null);
  };

  const handleDrop = (e: DragEvent, targetTab: ServerTab) => {
    e.preventDefault();
    setDragOverId(null);
    if (!dragSourceId || dragSourceId === targetTab.id) return;

    const tabsCopy = [...props.tabs];
    const sourceIdx = tabsCopy.findIndex((t) => t.id === dragSourceId);
    const targetIdx = tabsCopy.findIndex((t) => t.id === targetTab.id);
    if (sourceIdx === -1 || targetIdx === -1) return;

    const [moved] = tabsCopy.splice(sourceIdx, 1);
    tabsCopy.splice(targetIdx, 0, moved);

    if (props.onTabsReorder) {
      props.onTabsReorder(tabsCopy);
    }
    dragSourceId = null;
  };

  const handleDragEnd = () => {
    dragSourceId = null;
    setDragOverId(null);
  };

  return (
    <div class={styles.tabbar}>
      <For each={props.tabs}>
        {(tab) => (
          <button
            class={`${styles.tab} ${tab.active ? styles.active : ""} ${dragOverId() === tab.id ? styles.dragOver : ""}`}
            onClick={() => props.onTabSelect(tab.id)}
            onMouseDown={(e) => handleMiddleClick(e, tab)}
            onContextMenu={(e) => handleContextMenu(e, tab)}
            draggable={true}
            onDragStart={(e) => handleDragStart(e, tab)}
            onDragOver={(e) => handleDragOver(e, tab)}
            onDragLeave={handleDragLeave}
            onDrop={(e) => handleDrop(e, tab)}
            onDragEnd={handleDragEnd}
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
