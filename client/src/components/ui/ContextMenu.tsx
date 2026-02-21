import { createSignal, For, Show, onCleanup, onMount } from "solid-js";
import styles from "./ContextMenu.module.css";

export interface ContextMenuItem {
  id: string;
  label: string;
  icon?: string;
  disabled?: boolean;
  separator?: boolean;
  onClick?: () => void;
}

interface ContextMenuProps {
  items: ContextMenuItem[];
  x: number;
  y: number;
  onClose: () => void;
}

export function ContextMenu(props: ContextMenuProps) {
  let menuRef: HTMLDivElement | undefined;

  const handleClick = (item: ContextMenuItem) => {
    if (item.disabled) return;
    item.onClick?.();
    props.onClose();
  };

  const handleOutsideClick = (e: MouseEvent) => {
    if (menuRef && !menuRef.contains(e.target as Node)) {
      props.onClose();
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      props.onClose();
    }
  };

  onMount(() => {
    document.addEventListener("mousedown", handleOutsideClick);
    document.addEventListener("keydown", handleKeyDown);
  });

  onCleanup(() => {
    document.removeEventListener("mousedown", handleOutsideClick);
    document.removeEventListener("keydown", handleKeyDown);
  });

  // Position so anpassen, dass das Menu nicht aus dem Viewport ragt
  const adjustedPosition = () => {
    const menuWidth = 200;
    const menuHeight = props.items.length * 30;
    const x = props.x + menuWidth > window.innerWidth ? props.x - menuWidth : props.x;
    const y = props.y + menuHeight > window.innerHeight ? props.y - menuHeight : props.y;
    return { x: Math.max(0, x), y: Math.max(0, y) };
  };

  return (
    <div
      ref={menuRef}
      class={styles.menu}
      style={{
        left: `${adjustedPosition().x}px`,
        top: `${adjustedPosition().y}px`,
      }}
    >
      <For each={props.items}>
        {(item) => (
          <Show
            when={!item.separator}
            fallback={<div class={styles.separator} />}
          >
            <button
              class={`${styles.menuItem} ${item.disabled ? styles.disabled : ""}`}
              onClick={() => handleClick(item)}
              disabled={item.disabled}
            >
              <Show when={item.icon}>
                <span class={styles.menuIcon}>{item.icon}</span>
              </Show>
              <span class={styles.menuLabel}>{item.label}</span>
            </button>
          </Show>
        )}
      </For>
    </div>
  );
}

// Hook fuer ContextMenu-Zustand
export function createContextMenu() {
  const [menuState, setMenuState] = createSignal<{
    visible: boolean;
    x: number;
    y: number;
    items: ContextMenuItem[];
  }>({ visible: false, x: 0, y: 0, items: [] });

  const show = (e: MouseEvent, items: ContextMenuItem[]) => {
    e.preventDefault();
    e.stopPropagation();
    setMenuState({ visible: true, x: e.clientX, y: e.clientY, items });
  };

  const hide = () => {
    setMenuState((prev) => ({ ...prev, visible: false }));
  };

  return { menuState, show, hide };
}
