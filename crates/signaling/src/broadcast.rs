//! Event-Broadcaster – Sendet Events an alle relevanten Clients
//!
//! Der EventBroadcaster verwaltet die Send-Queues aller verbundenen Clients
//! und stellt Methoden bereit, um Nachrichten gezielt oder an alle zu senden.
//!
//! ## Selektives Broadcasting
//! - An alle Clients: `an_alle_senden`
//! - An einen Channel: `an_channel_senden`
//! - An spezifische User: `an_user_senden`
//! - An alle ausser einen: `an_alle_ausser_senden`

use dashmap::DashMap;
use speakeasy_core::types::{ChannelId, UserId};
use speakeasy_protocol::control::ControlMessage;
use std::sync::Arc;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Konfiguration
// ---------------------------------------------------------------------------

/// Groesse der Send-Queue pro Client
const SEND_QUEUE_GROESSE: usize = 64;

// ---------------------------------------------------------------------------
// ClientSender
// ---------------------------------------------------------------------------

/// Handle auf die Send-Queue eines verbundenen Clients
#[derive(Clone, Debug)]
pub struct ClientSender {
    pub user_id: UserId,
    pub tx: mpsc::Sender<ControlMessage>,
}

impl ClientSender {
    /// Sendet eine Nachricht nicht-blockierend an den Client
    ///
    /// Gibt `false` zurueck wenn die Queue voll oder geschlossen ist.
    pub fn senden(&self, nachricht: ControlMessage) -> bool {
        match self.tx.try_send(nachricht) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(user_id = %self.user_id, "Send-Queue voll – Nachricht verworfen");
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::debug!(user_id = %self.user_id, "Send-Queue geschlossen (Client getrennt)");
                false
            }
        }
    }
}

// ---------------------------------------------------------------------------
// EventBroadcaster
// ---------------------------------------------------------------------------

/// Zentraler Event-Broadcaster fuer alle verbundenen Clients
///
/// Thread-safe via Arc + DashMap. Clone teilt den inneren Zustand.
#[derive(Clone)]
pub struct EventBroadcaster {
    inner: Arc<EventBroadcasterInner>,
}

struct EventBroadcasterInner {
    /// Client-Sender, indiziert nach UserId
    clients: DashMap<UserId, ClientSender>,
    /// Channel-Mitgliedschaft: channel_id -> Vec<UserId>
    channel_members: DashMap<ChannelId, Vec<UserId>>,
}

impl EventBroadcaster {
    /// Erstellt einen neuen EventBroadcaster
    pub fn neu() -> Self {
        Self {
            inner: Arc::new(EventBroadcasterInner {
                clients: DashMap::new(),
                channel_members: DashMap::new(),
            }),
        }
    }

    /// Registriert einen neuen Client und gibt seine Empfangs-Queue zurueck
    ///
    /// Die `ClientConnection` liest aus dieser Queue und sendet via TCP.
    pub fn client_registrieren(&self, user_id: UserId) -> mpsc::Receiver<ControlMessage> {
        let (tx, rx) = mpsc::channel(SEND_QUEUE_GROESSE);
        let sender = ClientSender { user_id, tx };
        self.inner.clients.insert(user_id, sender);
        tracing::debug!(user_id = %user_id, "Client im Broadcaster registriert");
        rx
    }

    /// Entfernt einen Client aus dem Broadcaster
    pub fn client_entfernen(&self, user_id: &UserId) {
        self.inner.clients.remove(user_id);
        // Aus allen Channels entfernen
        self.inner.channel_members.iter_mut().for_each(|mut entry| {
            entry.value_mut().retain(|uid| uid != user_id);
        });
        // Leere Channel-Eintraege aufraumen
        self.inner.channel_members.retain(|_, members| !members.is_empty());
        tracing::debug!(user_id = %user_id, "Client aus Broadcaster entfernt");
    }

    /// Fuegt einen Client einem Channel hinzu (fuer selektives Broadcasting)
    pub fn channel_beitreten(&self, user_id: UserId, channel_id: ChannelId) {
        // Aus altem Channel entfernen
        self.inner.channel_members.iter_mut().for_each(|mut entry| {
            entry.value_mut().retain(|uid| uid != &user_id);
        });

        self.inner
            .channel_members
            .entry(channel_id)
            .or_default()
            .push(user_id);
    }

    /// Entfernt einen Client aus seinem Channel
    pub fn channel_verlassen(&self, user_id: &UserId) {
        self.inner.channel_members.iter_mut().for_each(|mut entry| {
            entry.value_mut().retain(|uid| uid != user_id);
        });
        self.inner.channel_members.retain(|_, members| !members.is_empty());
    }

    /// Sendet eine Nachricht an einen einzelnen Client
    ///
    /// Gibt `true` zurueck wenn der Client gefunden und die Nachricht eingereiht wurde.
    pub fn an_user_senden(&self, user_id: &UserId, nachricht: ControlMessage) -> bool {
        match self.inner.clients.get(user_id) {
            Some(sender) => sender.senden(nachricht),
            None => {
                tracing::debug!(user_id = %user_id, "Senden an unbekannten Client");
                false
            }
        }
    }

    /// Sendet eine Nachricht an alle Clients in einem Channel
    ///
    /// Gibt die Anzahl der erfolgreichen Sendungen zurueck.
    pub fn an_channel_senden(&self, channel_id: &ChannelId, nachricht: ControlMessage) -> usize {
        let user_ids = match self.inner.channel_members.get(channel_id) {
            Some(ids) => ids.clone(),
            None => return 0,
        };

        let mut gesendet = 0;
        for user_id in &user_ids {
            if let Some(sender) = self.inner.clients.get(user_id) {
                if sender.senden(nachricht.clone()) {
                    gesendet += 1;
                }
            }
        }
        gesendet
    }

    /// Sendet eine Nachricht an alle Clients in einem Channel ausser einem
    ///
    /// Nuetzlich um Join/Leave-Events zu verteilen ohne den Ausloeser zu informieren.
    pub fn an_channel_ausser_senden(
        &self,
        channel_id: &ChannelId,
        ausgeschlossen: &UserId,
        nachricht: ControlMessage,
    ) -> usize {
        let user_ids = match self.inner.channel_members.get(channel_id) {
            Some(ids) => ids.clone(),
            None => return 0,
        };

        let mut gesendet = 0;
        for user_id in &user_ids {
            if user_id == ausgeschlossen {
                continue;
            }
            if let Some(sender) = self.inner.clients.get(user_id) {
                if sender.senden(nachricht.clone()) {
                    gesendet += 1;
                }
            }
        }
        gesendet
    }

    /// Sendet eine Nachricht an alle verbundenen Clients
    ///
    /// Gibt die Anzahl der erfolgreichen Sendungen zurueck.
    pub fn an_alle_senden(&self, nachricht: ControlMessage) -> usize {
        let mut gesendet = 0;
        self.inner.clients.iter().for_each(|entry| {
            if entry.value().senden(nachricht.clone()) {
                gesendet += 1;
            }
        });
        gesendet
    }

    /// Sendet eine Nachricht an alle verbundenen Clients ausser einem
    pub fn an_alle_ausser_senden(
        &self,
        ausgeschlossen: &UserId,
        nachricht: ControlMessage,
    ) -> usize {
        let mut gesendet = 0;
        self.inner.clients.iter().for_each(|entry| {
            if entry.key() == ausgeschlossen {
                return;
            }
            if entry.value().senden(nachricht.clone()) {
                gesendet += 1;
            }
        });
        gesendet
    }

    /// Gibt die Anzahl der registrierten Clients zurueck
    pub fn client_anzahl(&self) -> usize {
        self.inner.clients.len()
    }

    /// Prueft ob ein Client registriert ist
    pub fn ist_registriert(&self, user_id: &UserId) -> bool {
        self.inner.clients.contains_key(user_id)
    }

    /// Gibt alle User-IDs in einem Channel zurueck
    pub fn user_ids_in_channel(&self, channel_id: &ChannelId) -> Vec<UserId> {
        self.inner
            .channel_members
            .get(channel_id)
            .map(|ids| ids.clone())
            .unwrap_or_default()
    }
}

impl Default for EventBroadcaster {
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
    

    fn test_nachricht(id: u32) -> ControlMessage {
        ControlMessage::ping(id, 12345)
    }

    #[tokio::test]
    async fn client_registrieren_und_senden() {
        let broadcaster = EventBroadcaster::neu();
        let uid = UserId::new();

        let mut rx = broadcaster.client_registrieren(uid);
        assert!(broadcaster.ist_registriert(&uid));

        let gesendet = broadcaster.an_user_senden(&uid, test_nachricht(1));
        assert!(gesendet);

        let empfangen = rx.try_recv().expect("Nachricht muss vorhanden sein");
        assert_eq!(empfangen.request_id, 1);
    }

    #[tokio::test]
    async fn an_channel_senden() {
        let broadcaster = EventBroadcaster::neu();
        let kanal = ChannelId::new();

        let uid1 = UserId::new();
        let uid2 = UserId::new();
        let uid3 = UserId::new(); // anderer Channel

        let mut rx1 = broadcaster.client_registrieren(uid1);
        let mut rx2 = broadcaster.client_registrieren(uid2);
        let mut rx3 = broadcaster.client_registrieren(uid3);

        broadcaster.channel_beitreten(uid1, kanal);
        broadcaster.channel_beitreten(uid2, kanal);
        // uid3 tritt keinem Channel bei

        let gesendet = broadcaster.an_channel_senden(&kanal, test_nachricht(10));
        assert_eq!(gesendet, 2);

        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
        assert!(rx3.try_recv().is_err(), "uid3 darf nichts empfangen");
    }

    #[tokio::test]
    async fn an_channel_ausser_senden() {
        let broadcaster = EventBroadcaster::neu();
        let kanal = ChannelId::new();

        let uid1 = UserId::new();
        let uid2 = UserId::new();

        let mut rx1 = broadcaster.client_registrieren(uid1);
        let mut rx2 = broadcaster.client_registrieren(uid2);

        broadcaster.channel_beitreten(uid1, kanal);
        broadcaster.channel_beitreten(uid2, kanal);

        // uid1 ist der Ausloeser und bekommt keine Nachricht
        broadcaster.an_channel_ausser_senden(&kanal, &uid1, test_nachricht(20));

        assert!(rx1.try_recv().is_err(), "Ausloeser darf nichts empfangen");
        assert!(rx2.try_recv().is_ok());
    }

    #[tokio::test]
    async fn an_alle_senden() {
        let broadcaster = EventBroadcaster::neu();

        let uids: Vec<UserId> = (0..5).map(|_| UserId::new()).collect();
        let mut receivers: Vec<_> = uids
            .iter()
            .map(|uid| broadcaster.client_registrieren(*uid))
            .collect();

        let gesendet = broadcaster.an_alle_senden(test_nachricht(99));
        assert_eq!(gesendet, 5);

        for rx in &mut receivers {
            assert!(rx.try_recv().is_ok());
        }
    }

    #[test]
    fn client_entfernen_bereinigt_channel_zugehoerigkeit() {
        let broadcaster = EventBroadcaster::neu();
        let kanal = ChannelId::new();
        let uid = UserId::new();

        let _rx = broadcaster.client_registrieren(uid);
        broadcaster.channel_beitreten(uid, kanal);
        assert_eq!(broadcaster.user_ids_in_channel(&kanal).len(), 1);

        broadcaster.client_entfernen(&uid);
        assert!(!broadcaster.ist_registriert(&uid));
        assert_eq!(broadcaster.user_ids_in_channel(&kanal).len(), 0);
    }
}
