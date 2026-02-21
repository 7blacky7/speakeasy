//! Gruppen-Schluessel-Verwaltung (Key Manager)
//!
//! Verwaltet Schluessel pro Channel:
//! - Erstellen neuer Schluessel bei Channel-Erstellung
//! - Verteilen an neue Mitglieder
//! - Rotation bei Join/Leave
//! - Widerruf bei Austritt

use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;

use crate::e2e::group_key::{create_group_key, rotate_group_key, wrap_key_for_recipient};
use crate::error::{CryptoError, CryptoResult};
use crate::types::{GroupKey, GroupKeyAlgorithm};

/// Verwaltet Gruppen-Schluessel fuer alle Channels
#[derive(Debug, Default)]
pub struct GroupKeyManager {
    /// Aktuelle Schluessel pro Channel (channel_id -> GroupKey)
    keys: DashMap<String, Arc<GroupKey>>,
    /// Widerrufene Schluessel (channel_id -> Vec<key_id>)
    revoked: DashMap<String, Vec<u64>>,
    /// Naechste Key-ID (monoton steigend)
    next_key_id: Arc<parking_lot::Mutex<u64>>,
}

impl GroupKeyManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Erstellt einen neuen Schluessel fuer einen Channel
    pub fn create_channel_key(
        &self,
        channel_id: &str,
        algorithm: GroupKeyAlgorithm,
    ) -> CryptoResult<Arc<GroupKey>> {
        let key_id = self.next_key_id();
        let key = create_group_key(channel_id, key_id, 0, algorithm)?;
        let key = Arc::new(key);
        self.keys.insert(channel_id.to_string(), Arc::clone(&key));
        Ok(key)
    }

    /// Gibt den aktuellen Schluessel fuer einen Channel zurueck
    pub fn get_channel_key(&self, channel_id: &str) -> CryptoResult<Arc<GroupKey>> {
        self.keys
            .get(channel_id)
            .map(|entry| Arc::clone(&*entry))
            .ok_or_else(|| CryptoError::KeinSchluessel {
                channel_id: channel_id.to_string(),
                epoch: 0,
            })
    }

    /// Rotiert den Schluessel eines Channels (bei Join/Leave)
    ///
    /// Gibt den neuen Schluessel zurueck.
    pub fn rotate_key(&self, channel_id: &str) -> CryptoResult<Arc<GroupKey>> {
        let current = self.get_channel_key(channel_id)?;
        let new_key = rotate_group_key(&current)?;
        let new_key = Arc::new(new_key);
        self.keys
            .insert(channel_id.to_string(), Arc::clone(&new_key));
        Ok(new_key)
    }

    /// Widerruft den aktuellen Schluessel fuer einen Channel
    ///
    /// Markiert den Schluessel als widerrufen. Danach muss `rotate_key`
    /// aufgerufen werden, um einen neuen Schluessel zu generieren.
    pub fn revoke_key(&self, channel_id: &str) -> CryptoResult<u64> {
        let current = self.get_channel_key(channel_id)?;
        let key_id = current.key_id;
        self.revoked
            .entry(channel_id.to_string())
            .or_default()
            .push(key_id);
        Ok(key_id)
    }

    /// Prueft ob ein Schluessel widerrufen ist
    pub fn is_revoked(&self, channel_id: &str, key_id: u64) -> bool {
        self.revoked
            .get(channel_id)
            .map(|list| list.contains(&key_id))
            .unwrap_or(false)
    }

    /// Verteilt den aktuellen Channel-Schluessel an eine Liste von Empfaengern
    ///
    /// Gibt eine Map user_id -> wrapped_key zurueck.
    /// Der wrapped_key ist mit dem oeffentlichen X25519-Schluessel des Empfaengers
    /// verschluesselt.
    pub fn distribute_key(
        &self,
        channel_id: &str,
        recipients: &HashMap<String, [u8; 32]>,
    ) -> CryptoResult<HashMap<String, Vec<u8>>> {
        let key = self.get_channel_key(channel_id)?;
        let mut result = HashMap::new();

        for (user_id, public_key) in recipients {
            let wrapped = wrap_key_for_recipient(&key, public_key)?;
            result.insert(user_id.clone(), wrapped);
        }

        Ok(result)
    }

    /// Entfernt einen Channel vollstaendig (bei Channel-Loesch)
    pub fn remove_channel(&self, channel_id: &str) {
        self.keys.remove(channel_id);
        self.revoked.remove(channel_id);
    }

    /// Gibt die aktuelle Epoch eines Channels zurueck
    pub fn current_epoch(&self, channel_id: &str) -> CryptoResult<u32> {
        Ok(self.get_channel_key(channel_id)?.epoch)
    }

    fn next_key_id(&self) -> u64 {
        let mut id = self.next_key_id.lock();
        *id += 1;
        *id
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
    use rand::rngs::OsRng;
    use rand::RngCore;

    fn random_x25519_pair() -> ([u8; 32], [u8; 32]) {
        let mut priv_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut priv_bytes);
        let private = StaticSecret::from(priv_bytes);
        let public = X25519PublicKey::from(&private);
        (priv_bytes, *public.as_bytes())
    }

    #[test]
    fn channel_key_erstellen() {
        let manager = GroupKeyManager::new();
        let key = manager
            .create_channel_key("ch-1", GroupKeyAlgorithm::Aes256Gcm)
            .unwrap();
        assert_eq!(key.channel_id, "ch-1");
        assert_eq!(key.epoch, 0);
        assert_eq!(key.key_bytes.len(), 32);
    }

    #[test]
    fn channel_key_abrufen() {
        let manager = GroupKeyManager::new();
        manager
            .create_channel_key("ch-2", GroupKeyAlgorithm::Aes256Gcm)
            .unwrap();
        let key = manager.get_channel_key("ch-2").unwrap();
        assert_eq!(key.channel_id, "ch-2");
    }

    #[test]
    fn fehlender_channel_ergibt_fehler() {
        let manager = GroupKeyManager::new();
        let result = manager.get_channel_key("nicht-vorhanden");
        assert!(result.is_err());
    }

    #[test]
    fn key_rotation_erhoehte_epoch() {
        let manager = GroupKeyManager::new();
        manager
            .create_channel_key("ch-3", GroupKeyAlgorithm::Aes256Gcm)
            .unwrap();
        let new_key = manager.rotate_key("ch-3").unwrap();
        assert_eq!(new_key.epoch, 1);
    }

    #[test]
    fn key_rotation_mehrfach() {
        let manager = GroupKeyManager::new();
        manager
            .create_channel_key("ch-4", GroupKeyAlgorithm::Aes256Gcm)
            .unwrap();
        manager.rotate_key("ch-4").unwrap();
        manager.rotate_key("ch-4").unwrap();
        let key = manager.get_channel_key("ch-4").unwrap();
        assert_eq!(key.epoch, 2);
    }

    #[test]
    fn key_revoke() {
        let manager = GroupKeyManager::new();
        manager
            .create_channel_key("ch-5", GroupKeyAlgorithm::Aes256Gcm)
            .unwrap();
        let revoked_id = manager.revoke_key("ch-5").unwrap();
        assert!(manager.is_revoked("ch-5", revoked_id));
    }

    #[test]
    fn nicht_widerrufener_key_ist_aktiv() {
        let manager = GroupKeyManager::new();
        let key = manager
            .create_channel_key("ch-6", GroupKeyAlgorithm::Aes256Gcm)
            .unwrap();
        assert!(!manager.is_revoked("ch-6", key.key_id));
    }

    #[test]
    fn key_distribution_an_empfaenger() {
        let manager = GroupKeyManager::new();
        manager
            .create_channel_key("ch-7", GroupKeyAlgorithm::Aes256Gcm)
            .unwrap();

        let (_, pub1) = random_x25519_pair();
        let (_, pub2) = random_x25519_pair();

        let mut recipients = HashMap::new();
        recipients.insert("user-1".to_string(), pub1);
        recipients.insert("user-2".to_string(), pub2);

        let distributed = manager.distribute_key("ch-7", &recipients).unwrap();
        assert_eq!(distributed.len(), 2);
        assert!(distributed.contains_key("user-1"));
        assert!(distributed.contains_key("user-2"));
        // Jeder Empfaenger erhaelt einen anderen wrapped key
        assert_ne!(
            distributed["user-1"],
            distributed["user-2"]
        );
    }

    #[test]
    fn channel_entfernen() {
        let manager = GroupKeyManager::new();
        manager
            .create_channel_key("ch-8", GroupKeyAlgorithm::Aes256Gcm)
            .unwrap();
        manager.remove_channel("ch-8");
        assert!(manager.get_channel_key("ch-8").is_err());
    }

    #[test]
    fn current_epoch() {
        let manager = GroupKeyManager::new();
        manager
            .create_channel_key("ch-9", GroupKeyAlgorithm::Aes256Gcm)
            .unwrap();
        assert_eq!(manager.current_epoch("ch-9").unwrap(), 0);
        manager.rotate_key("ch-9").unwrap();
        assert_eq!(manager.current_epoch("ch-9").unwrap(), 1);
    }

    #[test]
    fn monotone_key_ids() {
        let manager = GroupKeyManager::new();
        let k1 = manager
            .create_channel_key("ch-a", GroupKeyAlgorithm::Aes256Gcm)
            .unwrap();
        let k2 = manager
            .create_channel_key("ch-b", GroupKeyAlgorithm::Aes256Gcm)
            .unwrap();
        assert!(k2.key_id > k1.key_id);
    }
}
