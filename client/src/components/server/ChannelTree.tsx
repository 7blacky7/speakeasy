import { createSignal, For, Show } from "solid-js";
import type { ChannelInfo, ClientInfo } from "../../bridge";
import { ContextMenu, createContextMenu, type ContextMenuItem } from "../ui/ContextMenu";
import styles from "./ChannelTree.module.css";

// Channel mit Kindkanaelen (Baumstruktur)
export interface ChannelNode extends ChannelInfo {
  children: ChannelNode[];
  has_password: boolean;
  is_full: boolean;
  channel_type: "permanent" | "semi_permanent" | "temporary";
}

interface ChannelTreeProps {
  channels: ChannelNode[];
  currentChannelId: string | null;
  currentUserId: string | null;
  onChannelJoin: (channelId: string) => void;
  onChannelSelect: (channel: ChannelNode) => void;
  onChannelEdit?: (channelId: string) => void;
  onChannelDelete?: (channelId: string) => void;
  onSubchannelCreate?: (parentId: string) => void;
  onUserMessage?: (userId: string) => void;
  onUserPoke?: (userId: string) => void;
  onUserKick?: (userId: string) => void;
  onUserBan?: (userId: string) => void;
  onUserMove?: (userId: string) => void;
  // Server-Root Props
  serverName?: string;
  onlineClients?: number;
  maxClients?: number;
  onServerClick?: () => void;
  onServerEdit?: () => void;
  onChannelCreate?: () => void;
  onPermissions?: () => void;
  onAuditLog?: () => void;
  serverSelected?: boolean;
}

// Flache Channel-Liste in Baumstruktur umwandeln
export function buildChannelTree(channels: ChannelInfo[]): ChannelNode[] {
  const nodeMap = new Map<string, ChannelNode>();
  const roots: ChannelNode[] = [];

  // Alle Channels als Nodes anlegen
  for (const ch of channels) {
    nodeMap.set(ch.id, {
      ...ch,
      children: [],
      has_password: false,
      is_full: ch.max_clients > 0 && ch.clients.length >= ch.max_clients,
      channel_type: "permanent",
    });
  }

  // Baum aufbauen
  for (const ch of channels) {
    const node = nodeMap.get(ch.id)!;
    if (ch.parent_id && nodeMap.has(ch.parent_id)) {
      nodeMap.get(ch.parent_id)!.children.push(node);
    } else {
      roots.push(node);
    }
  }

  return roots;
}

export default function ChannelTree(props: ChannelTreeProps) {
  const { menuState, show: showMenu, hide: hideMenu } = createContextMenu();
  const [serverCollapsed, setServerCollapsed] = createSignal(false);

  const handleServerContextMenu = (e: MouseEvent) => {
    const items: ContextMenuItem[] = [
      { id: "create", label: "Channel erstellen", icon: "+", onClick: () => props.onChannelCreate?.(), disabled: !props.onChannelCreate },
      { id: "sep1", label: "", separator: true },
      { id: "edit", label: "Server bearbeiten", icon: "\u270E", onClick: () => props.onServerEdit?.(), disabled: !props.onServerEdit },
      { id: "permissions", label: "Berechtigungen verwalten", icon: "\u2261", onClick: () => props.onPermissions?.(), disabled: !props.onPermissions },
      { id: "audit", label: "Audit-Log anzeigen", icon: "\u2630", onClick: () => props.onAuditLog?.(), disabled: !props.onAuditLog },
    ];
    showMenu(e, items);
  };

  const handleServerClick = () => {
    props.onServerClick?.();
  };

  const handleServerToggle = (e: MouseEvent) => {
    e.stopPropagation();
    setServerCollapsed((v) => !v);
  };

  return (
    <div class={styles.tree}>
      {/* Server als Root-Element */}
      <Show when={props.serverName}>
        <div
          class={`${styles.serverRoot} ${props.serverSelected ? styles.serverRootSelected : ""}`}
          onClick={handleServerClick}
          onContextMenu={handleServerContextMenu}
        >
          <button class={styles.toggleBtn} onClick={handleServerToggle}>
            {serverCollapsed() ? "[+]" : "[-]"}
          </button>
          <span class={styles.serverIcon}>S</span>
          <span class={styles.serverRootName}>{props.serverName}</span>
          <Show when={props.maxClients !== undefined && props.maxClients! > 0}>
            <span class={styles.clientCount}>
              {props.onlineClients ?? 0}/{props.maxClients}
            </span>
          </Show>
        </div>
      </Show>

      {/* Channels darunter (eingerueckt wenn Server-Root vorhanden) */}
      <Show when={!serverCollapsed()}>
        <For each={props.channels}>
          {(channel) => (
            <ChannelBranch
              channel={channel}
              depth={props.serverName ? 1 : 0}
              currentChannelId={props.currentChannelId}
              currentUserId={props.currentUserId}
              onChannelJoin={props.onChannelJoin}
              onChannelSelect={props.onChannelSelect}
              onChannelEdit={props.onChannelEdit}
              onChannelDelete={props.onChannelDelete}
              onSubchannelCreate={props.onSubchannelCreate}
              onUserMessage={props.onUserMessage}
              onUserPoke={props.onUserPoke}
              onUserKick={props.onUserKick}
              onUserBan={props.onUserBan}
              onUserMove={props.onUserMove}
              showMenu={showMenu}
            />
          )}
        </For>
      </Show>

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

// --- Channel-Zweig (rekursiv) ---

interface ChannelBranchProps {
  channel: ChannelNode;
  depth: number;
  currentChannelId: string | null;
  currentUserId: string | null;
  onChannelJoin: (channelId: string) => void;
  onChannelSelect: (channel: ChannelNode) => void;
  onChannelEdit?: (channelId: string) => void;
  onChannelDelete?: (channelId: string) => void;
  onSubchannelCreate?: (parentId: string) => void;
  onUserMessage?: (userId: string) => void;
  onUserPoke?: (userId: string) => void;
  onUserKick?: (userId: string) => void;
  onUserBan?: (userId: string) => void;
  onUserMove?: (userId: string) => void;
  showMenu: (e: MouseEvent, items: ContextMenuItem[]) => void;
}

function ChannelBranch(props: ChannelBranchProps) {
  const [collapsed, setCollapsed] = createSignal(false);
  const ch = props.channel;
  const hasChildren = () => ch.children.length > 0 || ch.clients.length > 0;
  const isCurrent = () => props.currentChannelId === ch.id;

  const channelIcon = () => {
    if (ch.is_full) return "\u2715"; // X - voll
    if (ch.has_password) return "\u0052"; // Schloss-Ersatz (R fuer Restricted)
    return "\u266A"; // Noten-Symbol als Lautsprecher-Ersatz
  };

  const channelIconClass = () => {
    if (ch.is_full) return styles.iconFull;
    if (ch.has_password) return styles.iconLocked;
    return styles.iconNormal;
  };

  const handleDblClick = () => {
    props.onChannelJoin(ch.id);
  };

  const handleClick = () => {
    props.onChannelSelect(ch);
  };

  const handleToggle = (e: MouseEvent) => {
    e.stopPropagation();
    setCollapsed((v) => !v);
  };

  const handleContextMenu = (e: MouseEvent) => {
    const items: ContextMenuItem[] = [
      { id: "join", label: "Channel beitreten", icon: "\u25B6", onClick: () => props.onChannelJoin(ch.id) },
      { id: "sep1", label: "", separator: true },
      { id: "edit", label: "Channel bearbeiten", icon: "\u270E", onClick: () => props.onChannelEdit?.(ch.id), disabled: !props.onChannelEdit },
      { id: "subchannel", label: "Subchannel erstellen", icon: "+", onClick: () => props.onSubchannelCreate?.(ch.id), disabled: !props.onSubchannelCreate },
      { id: "sep2", label: "", separator: true },
      { id: "delete", label: "Channel loeschen", icon: "\u2212", onClick: () => props.onChannelDelete?.(ch.id), disabled: !props.onChannelDelete },
    ];
    props.showMenu(e, items);
  };

  return (
    <div class={styles.branch}>
      {/* Channel-Zeile */}
      <div
        class={`${styles.channelRow} ${isCurrent() ? styles.currentChannel : ""}`}
        style={{ "padding-left": `${8 + props.depth * 16}px` }}
        onClick={handleClick}
        onDblClick={handleDblClick}
        onContextMenu={handleContextMenu}
      >
        {/* Toggle-Button */}
        <Show
          when={hasChildren()}
          fallback={<span class={styles.toggleSpacer} />}
        >
          <button class={styles.toggleBtn} onClick={handleToggle}>
            {collapsed() ? "[+]" : "[-]"}
          </button>
        </Show>

        {/* Channel-Icon */}
        <span class={`${styles.channelIcon} ${channelIconClass()}`}>
          {channelIcon()}
        </span>

        {/* Channel-Name */}
        <span class={styles.channelName}>{ch.name}</span>

        {/* Client-Anzahl */}
        <Show when={ch.max_clients > 0}>
          <span class={styles.clientCount}>
            {ch.clients.length}/{ch.max_clients}
          </span>
        </Show>
      </div>

      {/* Kinder (Clients + Subchannels) */}
      <Show when={!collapsed()}>
        {/* Clients im Channel */}
        <For each={ch.clients}>
          {(client) => (
            <ClientEntry
              client={client}
              depth={props.depth + 1}
              isSelf={client.id === props.currentUserId}
              showMenu={props.showMenu}
              onMessage={props.onUserMessage}
              onPoke={props.onUserPoke}
              onKick={props.onUserKick}
              onBan={props.onUserBan}
              onMove={props.onUserMove}
            />
          )}
        </For>

        {/* Subchannels */}
        <For each={ch.children}>
          {(child) => (
            <ChannelBranch
              channel={child}
              depth={props.depth + 1}
              currentChannelId={props.currentChannelId}
              currentUserId={props.currentUserId}
              onChannelJoin={props.onChannelJoin}
              onChannelSelect={props.onChannelSelect}
              onChannelEdit={props.onChannelEdit}
              onChannelDelete={props.onChannelDelete}
              onSubchannelCreate={props.onSubchannelCreate}
              onUserMessage={props.onUserMessage}
              onUserPoke={props.onUserPoke}
              onUserKick={props.onUserKick}
              onUserBan={props.onUserBan}
              onUserMove={props.onUserMove}
              showMenu={props.showMenu}
            />
          )}
        </For>
      </Show>
    </div>
  );
}

// --- Client-Eintrag ---

interface ClientEntryProps {
  client: ClientInfo;
  depth: number;
  isSelf: boolean;
  showMenu: (e: MouseEvent, items: ContextMenuItem[]) => void;
  onMessage?: (userId: string) => void;
  onPoke?: (userId: string) => void;
  onKick?: (userId: string) => void;
  onBan?: (userId: string) => void;
  onMove?: (userId: string) => void;
}

function ClientEntry(props: ClientEntryProps) {
  const c = props.client;

  const statusIcon = () => {
    if (c.is_deafened) return "\u2298"; // Durchgestrichener Kreis
    if (c.is_muted) return "\u2300"; // Durchmesser-Symbol
    return "\u2022"; // Punkt
  };

  const statusClass = () => {
    if (c.is_deafened) return styles.statusDeafened;
    if (c.is_muted) return styles.statusMuted;
    return styles.statusOnline;
  };

  const handleContextMenu = (e: MouseEvent) => {
    const items: ContextMenuItem[] = [
      { id: "message", label: "Nachricht senden", icon: "\u2709", onClick: () => props.onMessage?.(c.id) },
      { id: "poke", label: "Anstupsen", icon: "!", onClick: () => props.onPoke?.(c.id) },
      { id: "sep1", label: "", separator: true },
      { id: "move", label: "Verschieben nach...", icon: "\u2192", onClick: () => props.onMove?.(c.id) },
      { id: "sep2", label: "", separator: true },
      { id: "kick", label: "Kicken", icon: "\u2716", onClick: () => props.onKick?.(c.id) },
      { id: "ban", label: "Bannen", icon: "\u26D4", onClick: () => props.onBan?.(c.id) },
    ];
    props.showMenu(e, items);
  };

  return (
    <div
      class={`${styles.clientRow} ${props.isSelf ? styles.selfClient : ""}`}
      style={{ "padding-left": `${24 + props.depth * 16}px` }}
      onContextMenu={handleContextMenu}
    >
      <span class={`${styles.clientStatus} ${statusClass()}`}>
        {statusIcon()}
      </span>
      <span class={styles.clientName}>
        {c.username}
        <Show when={props.isSelf}>
          <span class={styles.selfLabel}> (Du)</span>
        </Show>
      </span>
    </div>
  );
}
