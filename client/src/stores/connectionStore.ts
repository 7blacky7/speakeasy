import { createSignal } from "solid-js";

export interface ConnectionTab {
  id: string;
  name: string;
  address: string;
  port: number;
  username: string;
  password?: string;
  connected: boolean;
}

// Modul-Level Signals (globaler State)
const [tabs, setTabs] = createSignal<ConnectionTab[]>([
  { id: "default", name: "Nicht verbunden", address: "", port: 9001, username: "", connected: false }
]);
const [activeTabId, setActiveTabId] = createSignal("default");

export function getTabs(): ConnectionTab[] { return tabs(); }
export function getActiveTabId(): string { return activeTabId(); }
export function getActiveTab(): ConnectionTab | undefined { return tabs().find(t => t.id === activeTabId()); }

export function setActiveTab(id: string): void { setActiveTabId(id); }

export function addTab(tab: ConnectionTab): void {
  setTabs(prev => [...prev, tab]);
}

export function removeTab(id: string): void {
  const wasActive = activeTabId() === id;
  setTabs(prev => {
    const filtered = prev.filter(t => t.id !== id);
    return filtered.length === 0
      ? [{ id: crypto.randomUUID(), name: "Nicht verbunden", address: "", port: 9001, username: "", connected: false }]
      : filtered;
  });
  // Wenn aktiver Tab entfernt, wechsle zum ersten
  if (wasActive) {
    const remaining = tabs();
    if (remaining.length > 0) setActiveTabId(remaining[0].id);
  }
}

export function updateTab(id: string, patch: Partial<ConnectionTab>): void {
  setTabs(prev => prev.map(t => t.id === id ? { ...t, ...patch } : t));
}

export function reorderTabs(idOrder: string[]): void {
  setTabs(prev => {
    const sorted = [...prev].sort((a, b) => {
      const ai = idOrder.indexOf(a.id);
      const bi = idOrder.indexOf(b.id);
      return ai - bi;
    });
    return sorted;
  });
}
