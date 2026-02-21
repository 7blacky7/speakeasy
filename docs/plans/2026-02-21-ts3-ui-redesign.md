# TeamSpeak 3 UI - Exaktes Redesign

**Datum:** 2026-02-21
**Ziel:** UI exakt wie TeamSpeak 3 umbauen. Keine Buttons, alles per Menueleiste + Rechtsklick.

## Layout

```
+------------------------------------------------------------------+
| [Menueleiste] Server | Bookmarks | Einstellungen                 |
+------------------------------------------------------------------+
| [Tab: Mein Server]  [Tab: +]                                     |
+------------------------------------------------------------------+
| Channel-Baum (links ~250px)   | Server-Info / Channel-Info       |
|                                |                                  |
| v [S] Mein Speakeasy Server   | === Mein Speakeasy Server ===   |
|   | Willkommen! (Banner)       | Willkommensnachricht...          |
|   |                            | Version / Clients / Uptime       |
|   v Default Channel            | [Admin: Bearbeitungsfelder]      |
|     > Admin (spricht)          |                                  |
|     > Moritz                   |                                  |
|   > Gaming                     |                                  |
|   > AFK                        |                                  |
|     > Bob (away)               |                                  |
+--------------------------------+----------------------------------+
| [Chat unten, Strg+Enter ein/aus]                                  |
+------------------------------------------------------------------+
| [Mic] [Speaker] [Away]       Admin | Latenz: 24ms | Verbunden    |
+------------------------------------------------------------------+
```

## Menueleiste

- **Server**: Verbinden..., Trennen, Beenden
- **Bookmarks**: Bookmark hinzufuegen, [gespeicherte Server], Alle anzeigen
- **Einstellungen**: Sound (Input/Output/Lautstaerken), Plugins, Account

## Kontextmenue (Rechtsklick)

- **Server-Name**: Server bearbeiten, Berechtigungen, Audit-Log, Channel erstellen
- **Channel**: Beitreten, Bearbeiten, Subchannel erstellen, Loeschen
- **User**: Kick, Ban, Verschieben, Poke, Nachricht

## Aenderungen gegenueber aktuellem Stand

1. ENTFERNEN: "+ Channel" Button im Header
2. ENTFERNEN: "Admin" Link in Sidebar
3. ENTFERNEN: Separater Settings-Link in Sidebar
4. NEU: Menueleiste (Server, Bookmarks, Einstellungen)
5. NEU: Tab-System fuer mehrere Server
6. NEU: Server als Root-Element im Channel-Baum
7. NEU: Server-Info Panel rechts (Admin kann dort konfigurieren)
8. NEU: Auto-Join Default-Channel nach Login
9. Rechtsklick-Kontextmenue fuer ALLE Aktionen

## Betroffene Dateien

- client/src/components/Sidebar.tsx - Entfernen: Admin/Settings Links
- client/src/pages/ServerView.tsx - Komplett: Menueleiste, Tabs, Server-Root
- client/src/components/server/ChannelTree.tsx - Server als Root, Kontextmenue erweitern
- client/src/components/server/ChannelInfo.tsx - Server-Info Modus + Admin-Bearbeitung
- NEU: client/src/components/ui/MenuBar.tsx - TS3-Menueleiste
- NEU: client/src/components/ui/TabBar.tsx - Server-Tabs
- NEU: client/src/components/server/ServerInfoPanel.tsx - Server-Verwaltung rechts
