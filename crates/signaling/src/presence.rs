//! Presence-Manager – Verwaltet Online-Status und Channel-Zugehoerigkeit
//!
//! Wer ist online, in welchem Channel? Dieser Manager haelt den ephemeren
//! Zustand aller verbundenen Clients und benachrichtigt Subscriber bei
//! Aenderungen (Join/Leave/StatusChange).

use dashmap::DashMap;
use speakeasy_core::types::{ChannelId, UserId};
use std::sync::Arc;
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// Presence-Events
// ---------------------------------------------------------------------------

/// Events die der PresenceManager versendet
#[derive(Debug, Clone)]
pub enum PresenceEvent {
    /// Client hat sich verbunden
    ClientVerbunden { user_id: UserId, username: String },
    /// Client hat sich getrennt
    ClientGetrennt { user_id: UserId },
    /// Client ist einem Channel beigetreten
    ChannelBeigetreten {
        user_id: UserId,
        channel_id: ChannelId,
    },
    /// Client hat einen Channel verlassen
    ChannelVerlassen {
        user_id: UserId,
        channel_id: ChannelId,
    },
    /// Client wurde in einen anderen Channel verschoben
    ClientVerschoben {
        user_id: UserId,
        von_channel: Option<ChannelId>,
        zu_channel: ChannelId,
    },
    /// Status-Aenderung (Mute, Deaf, etc.)
    StatusGeaendert {
        user_id: UserId,
        is_input_muted: bool,
        is_output_muted: bool,
    },
}

// ---------------------------------------------------------------------------
// ClientPresence
// ---------------------------------------------------------------------------

/// Presence-Info eines verbundenen Clients
#[derive(Debug, Clone)]
pub struct ClientPresence {
    pub user_id: UserId,
    pub username: String,
    pub display_name: String,
    pub channel_id: Option<ChannelId>,
    pub is_input_muted: bool,
    pub is_output_muted: bool,
}

// ---------------------------------------------------------------------------
// PresenceManager
// ---------------------------------------------------------------------------

/// Groesse des Broadcast-Kanals fuer Presence-Events
const EVENT_KANAL_GROESSE: usize = 256;

/// Verwaltet den Online-Status aller verbundenen Clients
///
/// Thread-safe via Arc + DashMap. Clone des Managers teilt den inneren Zustand.
#[derive(Clone)]
pub struct PresenceManager {
    inner: Arc<PresenceManagerInner>,
}

struct PresenceManagerInner {
    /// Alle verbundenen Clients, indiziert nach UserId
    clients: DashMap<UserId, ClientPresence>,
    /// Channel -> Liste der User-IDs in diesem Channel
    channel_clients: DashMap<ChannelId, Vec<UserId>>,
    /// Broadcast-Sender fuer Presence-Events
    event_tx: broadcast::Sender<PresenceEvent>,
}

impl PresenceManager {
    /// Erstellt einen neuen PresenceManager
    pub fn neu() -> Self {
        let (event_tx, _) = broadcast::channel(EVENT_KANAL_GROESSE);
        Self {
            inner: Arc::new(PresenceManagerInner {
                clients: DashMap::new(),
                channel_clients: DashMap::new(),
                event_tx,
            }),
        }
    }

    /// Registriert einen neuen Client als online
    pub fn client_verbunden(&self, presence: ClientPresence) {
        let user_id = presence.user_id;
        let username = presence.username.clone();
        self.inner.clients.insert(user_id, presence);

        tracing::info!(user_id = %user_id, username = %username, "Client online");
        let _ = self
            .inner
            .event_tx
            .send(PresenceEvent::ClientVerbunden { user_id, username });
    }

    /// Entfernt einen Client (Verbindung getrennt)
    ///
    /// Entfernt den Client auch aus seinem Channel falls vorhanden.
    pub fn client_getrennt(&self, user_id: &UserId) {
        if let Some((_, presence)) = self.inner.clients.remove(user_id) {
            // Aus Channel entfernen falls vorhanden
            if let Some(channel_id) = presence.channel_id {
                self.aus_channel_entfernen_intern(user_id, &channel_id);
            }

            tracing::info!(user_id = %user_id, "Client offline");
            let _ = self
                .inner
                .event_tx
                .send(PresenceEvent::ClientGetrennt { user_id: *user_id });
        }
    }

    /// Fuegt einen Client einem Channel hinzu
    pub fn channel_beitreten(&self, user_id: UserId, channel_id: ChannelId) {
        let alter_channel = {
            let mut entry = match self.inner.clients.get_mut(&user_id) {
                Some(e) => e,
                None => {
                    tracing::warn!(user_id = %user_id, "Channel-Beitritt fuer unbekannten Client");
                    return;
                }
            };
            let alter = entry.channel_id;
            entry.channel_id = Some(channel_id);
            alter
        };

        // Aus altem Channel entfernen
        if let Some(alter) = alter_channel {
            if alter != channel_id {
                self.aus_channel_entfernen_intern(&user_id, &alter);
                let _ = self.inner.event_tx.send(PresenceEvent::ClientVerschoben {
                    user_id,
                    von_channel: Some(alter),
                    zu_channel: channel_id,
                });
            }
        } else {
            let _ = self.inner.event_tx.send(PresenceEvent::ChannelBeigetreten {
                user_id,
                channel_id,
            });
        }

        // Zum neuen Channel hinzufuegen
        self.inner
            .channel_clients
            .entry(channel_id)
            .or_default()
            .push(user_id);

        tracing::debug!(user_id = %user_id, channel_id = %channel_id, "Client Channel beigetreten");
    }

    /// Entfernt einen Client aus seinem Channel
    pub fn channel_verlassen(&self, user_id: &UserId) {
        let channel_id = {
            let mut entry = match self.inner.clients.get_mut(user_id) {
                Some(e) => e,
                None => return,
            };
            let c = entry.channel_id;
            entry.channel_id = None;
            c
        };

        if let Some(channel_id) = channel_id {
            self.aus_channel_entfernen_intern(user_id, &channel_id);
            let _ = self.inner.event_tx.send(PresenceEvent::ChannelVerlassen {
                user_id: *user_id,
                channel_id,
            });
            tracing::debug!(user_id = %user_id, channel_id = %channel_id, "Client Channel verlassen");
        }
    }

    /// Aktualisiert den Mute/Deaf-Status eines Clients
    pub fn status_aktualisieren(
        &self,
        user_id: UserId,
        is_input_muted: bool,
        is_output_muted: bool,
    ) {
        if let Some(mut entry) = self.inner.clients.get_mut(&user_id) {
            entry.is_input_muted = is_input_muted;
            entry.is_output_muted = is_output_muted;
        }

        let _ = self.inner.event_tx.send(PresenceEvent::StatusGeaendert {
            user_id,
            is_input_muted,
            is_output_muted,
        });
    }

    /// Gibt alle Clients in einem bestimmten Channel zurueck
    pub fn clients_in_channel(&self, channel_id: &ChannelId) -> Vec<ClientPresence> {
        let user_ids = match self.inner.channel_clients.get(channel_id) {
            Some(ids) => ids.clone(),
            None => return Vec::new(),
        };

        user_ids
            .iter()
            .filter_map(|uid| self.inner.clients.get(uid).map(|e| e.clone()))
            .collect()
    }

    /// Gibt alle online Clients zurueck
    pub fn alle_clients(&self) -> Vec<ClientPresence> {
        self.inner
            .clients
            .iter()
            .map(|e| e.value().clone())
            .collect()
    }

    /// Gibt die Presence-Info eines Clients zurueck
    pub fn client_presence(&self, user_id: &UserId) -> Option<ClientPresence> {
        self.inner.clients.get(user_id).map(|e| e.clone())
    }

    /// Prueft ob ein Client online ist
    pub fn ist_online(&self, user_id: &UserId) -> bool {
        self.inner.clients.contains_key(user_id)
    }

    /// Gibt die Anzahl der online Clients zurueck
    pub fn online_anzahl(&self) -> usize {
        self.inner.clients.len()
    }

    /// Gibt den aktuellen Channel eines Clients zurueck
    pub fn channel_von_client(&self, user_id: &UserId) -> Option<ChannelId> {
        self.inner.clients.get(user_id)?.channel_id
    }

    /// Gibt alle User-IDs in einem Channel zurueck
    pub fn user_ids_in_channel(&self, channel_id: &ChannelId) -> Vec<UserId> {
        self.inner
            .channel_clients
            .get(channel_id)
            .map(|ids| ids.clone())
            .unwrap_or_default()
    }

    /// Abonniert Presence-Events
    pub fn events_abonnieren(&self) -> broadcast::Receiver<PresenceEvent> {
        self.inner.event_tx.subscribe()
    }

    // -----------------------------------------------------------------------
    // Interne Hilfsmethoden
    // -----------------------------------------------------------------------

    fn aus_channel_entfernen_intern(&self, user_id: &UserId, channel_id: &ChannelId) {
        if let Some(mut ids) = self.inner.channel_clients.get_mut(channel_id) {
            ids.retain(|uid| uid != user_id);
            let ist_leer = ids.is_empty();
            drop(ids);
            if ist_leer {
                self.inner.channel_clients.remove(channel_id);
            }
        }
    }
}

impl Default for PresenceManager {
    fn default() -> Self {
        Self::neu()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_presence(user_id: UserId, name: &str) -> ClientPresence {
        ClientPresence {
            user_id,
            username: name.to_string(),
            display_name: name.to_string(),
            channel_id: None,
            is_input_muted: false,
            is_output_muted: false,
        }
    }

    #[test]
    fn client_verbinden_und_trennen() {
        let pm = PresenceManager::neu();
        let uid = UserId::new();

        pm.client_verbunden(test_presence(uid, "testuser"));
        assert!(pm.ist_online(&uid));
        assert_eq!(pm.online_anzahl(), 1);

        pm.client_getrennt(&uid);
        assert!(!pm.ist_online(&uid));
        assert_eq!(pm.online_anzahl(), 0);
    }

    #[test]
    fn channel_beitreten_und_verlassen() {
        let pm = PresenceManager::neu();
        let uid = UserId::new();
        let kanal = ChannelId::new();

        pm.client_verbunden(test_presence(uid, "user1"));
        pm.channel_beitreten(uid, kanal);

        assert_eq!(pm.channel_von_client(&uid), Some(kanal));
        assert_eq!(pm.clients_in_channel(&kanal).len(), 1);
        assert_eq!(pm.user_ids_in_channel(&kanal).len(), 1);

        pm.channel_verlassen(&uid);
        assert_eq!(pm.channel_von_client(&uid), None);
        assert_eq!(pm.clients_in_channel(&kanal).len(), 0);
    }

    #[test]
    fn mehrere_clients_in_channel() {
        let pm = PresenceManager::neu();
        let kanal = ChannelId::new();

        for i in 0..3 {
            let uid = UserId::new();
            pm.client_verbunden(test_presence(uid, &format!("user{}", i)));
            pm.channel_beitreten(uid, kanal);
        }

        assert_eq!(pm.clients_in_channel(&kanal).len(), 3);
        assert_eq!(pm.online_anzahl(), 3);
    }

    #[test]
    fn channel_wechsel_entfernt_aus_altem_channel() {
        let pm = PresenceManager::neu();
        let uid = UserId::new();
        let kanal_a = ChannelId::new();
        let kanal_b = ChannelId::new();

        pm.client_verbunden(test_presence(uid, "wechsler"));
        pm.channel_beitreten(uid, kanal_a);
        assert_eq!(pm.clients_in_channel(&kanal_a).len(), 1);

        pm.channel_beitreten(uid, kanal_b);
        assert_eq!(pm.clients_in_channel(&kanal_a).len(), 0);
        assert_eq!(pm.clients_in_channel(&kanal_b).len(), 1);
        assert_eq!(pm.channel_von_client(&uid), Some(kanal_b));
    }

    #[test]
    fn clone_teilt_inneren_state() {
        let pm1 = PresenceManager::neu();
        let pm2 = pm1.clone();
        let uid = UserId::new();

        pm1.client_verbunden(test_presence(uid, "shared"));
        assert!(pm2.ist_online(&uid));
    }

    #[tokio::test]
    async fn events_werden_versendet() {
        let pm = PresenceManager::neu();
        let mut rx = pm.events_abonnieren();
        let uid = UserId::new();

        pm.client_verbunden(test_presence(uid, "event_user"));

        let event = rx.try_recv().expect("Event muss vorhanden sein");
        assert!(matches!(event, PresenceEvent::ClientVerbunden { .. }));
    }
}
