export interface ConnectionEntry {
  address: string;
  port: number;
  username: string;
  lastUsed: number; // Date.now() timestamp
}

const HISTORY_KEY = "speakeasy-connection-history";
const MAX_ENTRIES = 20;

export function loadHistory(): ConnectionEntry[] {
  try {
    const stored = localStorage.getItem(HISTORY_KEY);
    const entries: ConnectionEntry[] = stored ? JSON.parse(stored) : [];
    return entries.sort((a, b) => b.lastUsed - a.lastUsed);
  } catch {
    return [];
  }
}

export function saveConnection(
  address: string,
  port: number,
  username: string,
): void {
  const entries = loadHistory();
  const idx = entries.findIndex(
    (e) => e.address === address && e.port === port,
  );
  const entry: ConnectionEntry = {
    address,
    port,
    username,
    lastUsed: Date.now(),
  };
  if (idx >= 0) {
    entries[idx] = entry;
  } else {
    entries.unshift(entry);
  }
  localStorage.setItem(
    HISTORY_KEY,
    JSON.stringify(entries.slice(0, MAX_ENTRIES)),
  );
}

export function getUniqueAddresses(): string[] {
  return [...new Set(loadHistory().map((e) => e.address))];
}

export function findEntry(address: string): ConnectionEntry | undefined {
  return loadHistory().find((e) => e.address === address);
}
