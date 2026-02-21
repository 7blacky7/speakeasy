//! Voice-State – In-Memory Zustand aller aktiven Voice-Sessions
//!
//! Verwaltet pro Client:
//! - SSRC und UDP-Endpunkt
//! - Channel-Zugehoerigkeit
//! - Codec-Konfiguration
//! - Speaking-Status
//! - Netzwerk-Statistiken
//!
//! Thread-safe durch DashMap (lock-free concurrent HashMap).

use dashmap::DashMap;
use speakeasy_core::types::{ChannelId, UserId};
use speakeasy_protocol::codec::OpusConfig;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// ClientVoiceState
// ---------------------------------------------------------------------------

/// Zustand eines einzelnen verbundenen Voice-Clients
#[derive(Debug, Clone)]
pub struct ClientVoiceState {
    /// Benutzer-ID
    pub user_id: UserId,
    /// Synchronisation Source – eindeutige Senderkennung (aus dem UDP-Header)
    pub ssrc: u32,
    /// UDP-Endpunkt des Clients
    pub udp_endpunkt: SocketAddr,
    /// Aktueller Voice-Kanal (None wenn nicht in einem Kanal)
    pub kanal_id: Option<ChannelId>,
    /// Vereinbarte Codec-Konfiguration
    pub codec_config: Option<OpusConfig>,
    /// Spricht der Client gerade?
    pub spricht: bool,
    /// Zeitpunkt des letzten empfangenen Pakets
    pub letztes_paket: Instant,
    /// Letzte gemessene RTT in ms
    pub rtt_ms: u32,
    /// Paketverlust-Rate (0.0–1.0)
    pub verlust_rate: f64,
    /// Gemessener Jitter in Ticks
    pub jitter_ticks: u32,
    /// Empfohlene Bitrate (kbps) – kann vom Congestion Controller angepasst werden
    pub empfohlene_bitrate_kbps: u16,
}

impl ClientVoiceState {
    /// Erstellt einen neuen Client-Zustand
    pub fn neu(user_id: UserId, ssrc: u32, udp_endpunkt: SocketAddr) -> Self {
        Self {
            user_id,
            ssrc,
            udp_endpunkt,
            kanal_id: None,
            codec_config: None,
            spricht: false,
            letztes_paket: Instant::now(),
            rtt_ms: 0,
            verlust_rate: 0.0,
            jitter_ticks: 0,
            empfohlene_bitrate_kbps: 64,
        }
    }

    /// Prueft ob der Client als inaktiv gilt (kein Paket seit `timeout`)
    pub fn ist_inaktiv(&self, timeout: Duration) -> bool {
        self.letztes_paket.elapsed() > timeout
    }

    /// Aktualisiert den Zeitstempel des letzten Pakets
    pub fn paket_empfangen(&mut self) {
        self.letztes_paket = Instant::now();
    }
}

// ---------------------------------------------------------------------------
// VoiceState
// ---------------------------------------------------------------------------

/// Timeout fuer inaktive Clients (30 Sekunden ohne Paket)
pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(30);

/// Zentraler In-Memory Voice-State aller aktiven Sessions
///
/// Thread-safe durch DashMap – concurrent reads ohne Lock.
/// Einzel-Eintraege werden durch per-Entry-Lock geschuetzt.
#[derive(Clone)]
pub struct VoiceState {
    inner: Arc<VoiceStateInner>,
}

struct VoiceStateInner {
    /// Client-States, indexiert nach UserId
    clients: DashMap<UserId, ClientVoiceState>,
    /// SSRC -> UserId Mapping fuer schnellen Lookup aus UDP-Paketen
    ssrc_index: DashMap<u32, UserId>,
    /// UDP-Endpunkt -> UserId Mapping
    endpunkt_index: DashMap<SocketAddr, UserId>,
}

impl VoiceState {
    /// Erstellt einen neuen leeren VoiceState
    pub fn neu() -> Self {
        Self {
            inner: Arc::new(VoiceStateInner {
                clients: DashMap::new(),
                ssrc_index: DashMap::new(),
                endpunkt_index: DashMap::new(),
            }),
        }
    }

    /// Registriert einen neuen Client
    pub fn client_registrieren(
        &self,
        user_id: UserId,
        ssrc: u32,
        udp_endpunkt: SocketAddr,
    ) {
        let state = ClientVoiceState::neu(user_id, ssrc, udp_endpunkt);
        self.inner.clients.insert(user_id, state);
        self.inner.ssrc_index.insert(ssrc, user_id);
        self.inner.endpunkt_index.insert(udp_endpunkt, user_id);
        tracing::info!(
            user_id = %user_id,
            ssrc,
            endpunkt = %udp_endpunkt,
            "Client registriert"
        );
    }

    /// Entfernt einen Client und bereinigt alle Indizes
    pub fn client_entfernen(&self, user_id: &UserId) -> Option<ClientVoiceState> {
        if let Some((_, state)) = self.inner.clients.remove(user_id) {
            self.inner.ssrc_index.remove(&state.ssrc);
            self.inner.endpunkt_index.remove(&state.udp_endpunkt);
            tracing::info!(user_id = %user_id, "Client entfernt");
            Some(state)
        } else {
            None
        }
    }

    /// Sucht UserId anhand der SSRC (Hot Path – DashMap read lock-free)
    pub fn user_id_von_ssrc(&self, ssrc: u32) -> Option<UserId> {
        self.inner.ssrc_index.get(&ssrc).map(|r| *r)
    }

    /// Sucht UserId anhand des UDP-Endpunkts
    pub fn user_id_von_endpunkt(&self, endpunkt: &SocketAddr) -> Option<UserId> {
        self.inner.endpunkt_index.get(endpunkt).map(|r| *r)
    }

    /// Gibt eine Referenz auf den Client-State zurueck (shared lock)
    pub fn client_state(&self, user_id: &UserId) -> Option<dashmap::mapref::one::Ref<'_, UserId, ClientVoiceState>> {
        self.inner.clients.get(user_id)
    }

    /// Aktualisiert den Client-State mit einer Closure
    pub fn client_aktualisieren<F>(&self, user_id: &UserId, f: F) -> bool
    where
        F: FnOnce(&mut ClientVoiceState),
    {
        if let Some(mut entry) = self.inner.clients.get_mut(user_id) {
            f(&mut entry);
            true
        } else {
            false
        }
    }

    /// Setzt den Kanal fuer einen Client
    pub fn kanal_setzen(&self, user_id: &UserId, kanal_id: Option<ChannelId>) {
        self.client_aktualisieren(user_id, |s| s.kanal_id = kanal_id);
    }

    /// Setzt den Speaking-Status
    pub fn speaking_setzen(&self, user_id: &UserId, spricht: bool) {
        self.client_aktualisieren(user_id, |s| s.spricht = spricht);
    }

    /// Aktualisiert den Paket-Zeitstempel (beim Empfang eines Pakets)
    pub fn paket_zeitstempel_aktualisieren(&self, user_id: &UserId) {
        self.client_aktualisieren(user_id, |s| s.paket_empfangen());
    }

    /// Gibt alle Clients in einem bestimmten Kanal zurueck
    ///
    /// Iteriert ueber DashMap – wird nicht im Hot Path verwendet
    pub fn clients_in_kanal(&self, kanal_id: &ChannelId) -> Vec<UserId> {
        self.inner
            .clients
            .iter()
            .filter_map(|entry| {
                if entry.kanal_id.as_ref() == Some(kanal_id) {
                    Some(entry.user_id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Gibt alle sprechenden Clients zurueck (Presence-Info)
    pub fn sprechende_clients(&self) -> Vec<(UserId, Option<ChannelId>)> {
        self.inner
            .clients
            .iter()
            .filter(|e| e.spricht)
            .map(|e| (e.user_id, e.kanal_id))
            .collect()
    }

    /// Bereinigt inaktive Clients (Timeout-Handling)
    ///
    /// Gibt die Liste der entfernten User-IDs zurueck.
    pub fn inaktive_bereinigen(&self, timeout: Duration) -> Vec<UserId> {
        let inaktive: Vec<UserId> = self
            .inner
            .clients
            .iter()
            .filter(|e| e.ist_inaktiv(timeout))
            .map(|e| e.user_id)
            .collect();

        for uid in &inaktive {
            self.client_entfernen(uid);
            tracing::warn!(user_id = %uid, "Inaktiver Client entfernt (Timeout)");
        }

        inaktive
    }

    /// Gibt die Anzahl der registrierten Clients zurueck
    pub fn client_anzahl(&self) -> usize {
        self.inner.clients.len()
    }

    /// Prueft ob ein Client registriert ist
    pub fn ist_registriert(&self, user_id: &UserId) -> bool {
        self.inner.clients.contains_key(user_id)
    }
}

impl Default for VoiceState {
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
    use std::net::{IpAddr, Ipv4Addr};

    fn test_endpunkt(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
    }

    #[test]
    fn client_registrieren_und_abfragen() {
        let state = VoiceState::neu();
        let uid = UserId::new();
        let endpunkt = test_endpunkt(10000);

        state.client_registrieren(uid, 0xCAFE, endpunkt);

        assert!(state.ist_registriert(&uid));
        assert_eq!(state.client_anzahl(), 1);

        // SSRC-Lookup
        let gefunden = state.user_id_von_ssrc(0xCAFE);
        assert_eq!(gefunden, Some(uid));

        // Endpunkt-Lookup
        let gefunden = state.user_id_von_endpunkt(&endpunkt);
        assert_eq!(gefunden, Some(uid));
    }

    #[test]
    fn client_entfernen_bereinigt_indizes() {
        let state = VoiceState::neu();
        let uid = UserId::new();
        let endpunkt = test_endpunkt(10001);

        state.client_registrieren(uid, 0x1234, endpunkt);
        state.client_entfernen(&uid);

        assert!(!state.ist_registriert(&uid));
        assert!(state.user_id_von_ssrc(0x1234).is_none());
        assert!(state.user_id_von_endpunkt(&endpunkt).is_none());
        assert_eq!(state.client_anzahl(), 0);
    }

    #[test]
    fn kanal_setzen_und_abfragen() {
        let state = VoiceState::neu();
        let uid = UserId::new();
        let kanal = ChannelId::new();

        state.client_registrieren(uid, 1, test_endpunkt(10002));
        state.kanal_setzen(&uid, Some(kanal));

        let clients = state.clients_in_kanal(&kanal);
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0], uid);
    }

    #[test]
    fn mehrere_clients_in_kanal() {
        let state = VoiceState::neu();
        let kanal = ChannelId::new();
        let anderer_kanal = ChannelId::new();

        for i in 0..3u16 {
            let uid = UserId::new();
            state.client_registrieren(uid, i as u32, test_endpunkt(10010 + i));
            state.kanal_setzen(&uid, Some(kanal));
        }

        // Ein Client in anderem Kanal
        let uid_other = UserId::new();
        state.client_registrieren(uid_other, 99, test_endpunkt(10020));
        state.kanal_setzen(&uid_other, Some(anderer_kanal));

        assert_eq!(state.clients_in_kanal(&kanal).len(), 3);
        assert_eq!(state.clients_in_kanal(&anderer_kanal).len(), 1);
    }

    #[test]
    fn speaking_status() {
        let state = VoiceState::neu();
        let uid = UserId::new();
        state.client_registrieren(uid, 1, test_endpunkt(10030));

        state.speaking_setzen(&uid, true);
        let sprechende = state.sprechende_clients();
        assert_eq!(sprechende.len(), 1);

        state.speaking_setzen(&uid, false);
        let sprechende = state.sprechende_clients();
        assert!(sprechende.is_empty());
    }

    #[test]
    fn clone_teilt_inneren_state() {
        let state1 = VoiceState::neu();
        let state2 = state1.clone();

        let uid = UserId::new();
        state1.client_registrieren(uid, 1, test_endpunkt(10040));

        // state2 sollte denselben Client sehen (Arc)
        assert!(state2.ist_registriert(&uid));
    }
}
