//! Plugin Registry – verwaltet installierte Plugins und ihre Metadaten

use dashmap::DashMap;
use std::path::PathBuf;

use crate::error::{PluginError, Result};
use crate::types::{PluginId, PluginInfo, PluginState, TrustLevel};

/// Eintrag in der Registry
#[derive(Debug, Clone)]
pub struct RegistryEintrag {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub pfad: PathBuf,
    pub state: PluginState,
    pub trust_level: TrustLevel,
    pub autostart: bool,
}

/// Plugin Registry – thread-sicher via DashMap
pub struct PluginRegistry {
    eintraege: DashMap<PluginId, RegistryEintrag>,
    /// Name -> ID Lookup
    name_index: DashMap<String, PluginId>,
}

impl PluginRegistry {
    /// Erstellt eine neue leere Registry
    pub fn neu() -> Self {
        Self {
            eintraege: DashMap::new(),
            name_index: DashMap::new(),
        }
    }

    /// Registriert ein Plugin
    pub fn registrieren(
        &self,
        id: PluginId,
        name: String,
        version: String,
        pfad: PathBuf,
        trust_level: TrustLevel,
    ) -> Result<()> {
        if self.name_index.contains_key(&name) {
            return Err(PluginError::BereitsGeladen(name));
        }

        let eintrag = RegistryEintrag {
            id,
            name: name.clone(),
            version,
            pfad,
            state: PluginState::Geladen,
            trust_level,
            autostart: false,
        };

        self.eintraege.insert(id, eintrag);
        self.name_index.insert(name, id);
        Ok(())
    }

    /// Entfernt ein Plugin aus der Registry
    pub fn entfernen(&self, id: PluginId) -> Result<RegistryEintrag> {
        let (_, eintrag) = self.eintraege.remove(&id)
            .ok_or_else(|| PluginError::NichtGefunden(id.to_string()))?;
        self.name_index.remove(&eintrag.name);
        Ok(eintrag)
    }

    /// Sucht ein Plugin per ID
    pub fn per_id(&self, id: PluginId) -> Option<RegistryEintrag> {
        self.eintraege.get(&id).map(|e| e.clone())
    }

    /// Sucht ein Plugin per Name
    pub fn per_name(&self, name: &str) -> Option<RegistryEintrag> {
        let id = self.name_index.get(name)?;
        self.eintraege.get(&*id).map(|e| e.clone())
    }

    /// Aktualisiert den Zustand eines Plugins
    pub fn zustand_setzen(&self, id: PluginId, state: PluginState) -> Result<()> {
        let mut eintrag = self.eintraege.get_mut(&id)
            .ok_or_else(|| PluginError::NichtGefunden(id.to_string()))?;
        eintrag.state = state;
        Ok(())
    }

    /// Gibt alle Eintraege als Liste zurueck
    pub fn alle(&self) -> Vec<RegistryEintrag> {
        self.eintraege.iter().map(|e| e.value().clone()).collect()
    }

    /// Gibt alle aktiven Plugins zurueck
    pub fn aktive(&self) -> Vec<RegistryEintrag> {
        self.eintraege
            .iter()
            .filter(|e| e.state == PluginState::Aktiv)
            .map(|e| e.value().clone())
            .collect()
    }

    /// Anzahl registrierter Plugins
    pub fn anzahl(&self) -> usize {
        self.eintraege.len()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::neu()
    }
}

/// Konvertiert einen RegistryEintrag in PluginInfo (fuer API/UI)
pub fn eintrag_zu_info(
    eintrag: &RegistryEintrag,
    geladen_am: chrono::DateTime<chrono::Utc>,
) -> PluginInfo {
    PluginInfo {
        id: eintrag.id,
        name: eintrag.name.clone(),
        version: eintrag.version.clone(),
        author: String::new(),
        description: String::new(),
        state: eintrag.state.clone(),
        trust_level: eintrag.trust_level.clone(),
        geladen_am,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_eintrag(name: &str) -> (PluginId, String, String, PathBuf, TrustLevel) {
        (
            PluginId::new(),
            name.to_string(),
            "1.0.0".to_string(),
            PathBuf::from(format!("/plugins/{}", name)),
            TrustLevel::NichtSigniert,
        )
    }

    #[test]
    fn registrieren_und_finden() {
        let registry = PluginRegistry::neu();
        let (id, name, version, pfad, trust) = test_eintrag("test-plugin");

        registry.registrieren(id, name.clone(), version, pfad, trust).unwrap();

        let e = registry.per_id(id).unwrap();
        assert_eq!(e.name, name);

        let e2 = registry.per_name(&name).unwrap();
        assert_eq!(e2.id, id);
    }

    #[test]
    fn doppeltes_registrieren_fehlschlaegt() {
        let registry = PluginRegistry::neu();
        let (id1, name, version, pfad, trust) = test_eintrag("doppelt");
        registry
            .registrieren(id1, name.clone(), version.clone(), pfad.clone(), trust.clone())
            .unwrap();

        let id2 = PluginId::new();
        let err = registry
            .registrieren(id2, name, version, pfad, trust)
            .unwrap_err();
        assert!(matches!(err, PluginError::BereitsGeladen(_)));
    }

    #[test]
    fn entfernen_ok() {
        let registry = PluginRegistry::neu();
        let (id, name, version, pfad, trust) = test_eintrag("zu-entfernen");
        registry.registrieren(id, name.clone(), version, pfad, trust).unwrap();

        registry.entfernen(id).unwrap();
        assert!(registry.per_id(id).is_none());
        assert!(registry.per_name(&name).is_none());
    }

    #[test]
    fn entfernen_nicht_gefunden() {
        let registry = PluginRegistry::neu();
        let err = registry.entfernen(PluginId::new()).unwrap_err();
        assert!(matches!(err, PluginError::NichtGefunden(_)));
    }

    #[test]
    fn zustand_setzen() {
        let registry = PluginRegistry::neu();
        let (id, name, version, pfad, trust) = test_eintrag("state-test");
        registry.registrieren(id, name, version, pfad, trust).unwrap();

        registry.zustand_setzen(id, PluginState::Aktiv).unwrap();
        assert_eq!(registry.per_id(id).unwrap().state, PluginState::Aktiv);
    }

    #[test]
    fn aktive_plugins_gefiltert() {
        let registry = PluginRegistry::neu();
        let (id1, name1, v1, p1, t1) = test_eintrag("aktiv");
        let (id2, name2, v2, p2, t2) = test_eintrag("inaktiv");
        registry.registrieren(id1, name1, v1, p1, t1).unwrap();
        registry.registrieren(id2, name2, v2, p2, t2).unwrap();

        registry.zustand_setzen(id1, PluginState::Aktiv).unwrap();

        let aktive = registry.aktive();
        assert_eq!(aktive.len(), 1);
        assert_eq!(aktive[0].id, id1);
    }

    #[test]
    fn anzahl_korrekt() {
        let registry = PluginRegistry::neu();
        assert_eq!(registry.anzahl(), 0);
        let (id, name, v, p, t) = test_eintrag("eins");
        registry.registrieren(id, name, v, p, t).unwrap();
        assert_eq!(registry.anzahl(), 1);
    }
}
