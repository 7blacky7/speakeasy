//! Permission-Engine fuer Speakeasy
//!
//! Implementiert die Aufloesungslogik fuer das mehrstufige Berechtigungssystem.
//!
//! Aufloesung (hoechste Prioritaet zuerst):
//!   1. Individuelle Berechtigungen des Users
//!   2. Kanal-Gruppe des Users in diesem Kanal
//!   3. Kanal-Default (Standardberechtigungen des Kanals)
//!   4. Server-Gruppen des Users (nach Prioritaet absteigend)
//!   5. Server-Default (globale Standardberechtigungen)
//!
//! Merge-Regel bei Konflikten auf gleicher Ebene: Deny > Grant > Skip

use std::collections::HashMap;

use crate::models::{BerechtigungsWert, TriState};

/// Ergebnis der Berechtigungsauflosung fuer einen Key
#[derive(Debug, Clone)]
pub struct AufgeloesteBerechtigung {
    pub permission_key: String,
    pub wert: BerechtigungsWert,
    /// Welche Stufe hat diese Berechtigung geliefert
    pub stufe: BerechtigungsStufe,
}

/// Hierarchie-Stufe einer aufgeloesten Berechtigung
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BerechtigungsStufe {
    Individual,
    KanalGruppe,
    KanalDefault,
    ServerGruppe { name: String },
    ServerDefault,
}

impl std::fmt::Display for BerechtigungsStufe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Individual => write!(f, "Individual"),
            Self::KanalGruppe => write!(f, "KanalGruppe"),
            Self::KanalDefault => write!(f, "KanalDefault"),
            Self::ServerGruppe { name } => write!(f, "ServerGruppe({name})"),
            Self::ServerDefault => write!(f, "ServerDefault"),
        }
    }
}

/// Eingabe fuer die Berechtigungsauflosung
#[derive(Debug, Clone)]
pub struct BerechtigungsEingabe {
    /// Individuelle Berechtigungen des Users
    pub individual: Vec<(String, BerechtigungsWert)>,
    /// Berechtigung aus der Kanal-Gruppe (falls vorhanden)
    pub kanal_gruppe: Option<Vec<(String, BerechtigungsWert)>>,
    /// Kanal-Default-Berechtigungen
    pub kanal_default: Vec<(String, BerechtigungsWert)>,
    /// Server-Gruppen-Berechtigungen (nach Prioritaet absteigend sortiert)
    pub server_gruppen: Vec<(String, Vec<(String, BerechtigungsWert)>)>,
    /// Server-Default-Berechtigungen
    pub server_default: Vec<(String, BerechtigungsWert)>,
}

/// Fuehrt die vollstaendige Berechtigungsauflosung durch
///
/// Gibt eine Map von permission_key -> AufgeloesteBerechtigung zurueck.
/// Berechtigungen mit Skip auf allen Ebenen werden ausgelassen.
pub fn berechtigungen_aufloesen(
    eingabe: &BerechtigungsEingabe,
) -> HashMap<String, AufgeloesteBerechtigung> {
    let mut ergebnis: HashMap<String, AufgeloesteBerechtigung> = HashMap::new();

    // Hilfsfunktion: Fuegt Berechtigungen einer Stufe hinzu, wenn der Key noch nicht gesetzt ist
    let mut setze_stufe = |perms: &[(String, BerechtigungsWert)], stufe: BerechtigungsStufe| {
        for (key, wert) in perms {
            if !ergebnis.contains_key(key) {
                // Skip-Werte werden nicht als "gesetzt" gewertet
                if ist_aktiv(wert) {
                    ergebnis.insert(
                        key.clone(),
                        AufgeloesteBerechtigung {
                            permission_key: key.clone(),
                            wert: wert.clone(),
                            stufe: stufe.clone(),
                        },
                    );
                }
            }
        }
    };

    // Stufe 1: Individual
    setze_stufe(&eingabe.individual, BerechtigungsStufe::Individual);

    // Stufe 2: Kanal-Gruppe
    if let Some(ref kg) = eingabe.kanal_gruppe {
        setze_stufe(kg, BerechtigungsStufe::KanalGruppe);
    }

    // Stufe 3: Kanal-Default
    setze_stufe(&eingabe.kanal_default, BerechtigungsStufe::KanalDefault);

    // Stufe 4: Server-Gruppen (nach Prioritaet sortiert, bereits sortiert in der Eingabe)
    for (name, perms) in &eingabe.server_gruppen {
        setze_stufe(
            perms,
            BerechtigungsStufe::ServerGruppe { name: name.clone() },
        );
    }

    // Stufe 5: Server-Default
    setze_stufe(&eingabe.server_default, BerechtigungsStufe::ServerDefault);

    ergebnis
}

/// Prueft ob ein Berechtigungswert "aktiv" ist (nicht Skip)
fn ist_aktiv(wert: &BerechtigungsWert) -> bool {
    match wert {
        BerechtigungsWert::TriState(ts) => *ts != TriState::Skip,
        BerechtigungsWert::IntLimit(_) => true,
        BerechtigungsWert::Scope(s) => !s.is_empty(),
    }
}

/// Merged mehrere Berechtigungswerte auf gleicher Ebene
///
/// Merge-Regel: Deny > Grant > Skip
/// Bei IntLimit und Scope: restriktivster Wert gewinnt
pub fn merge_werte(werte: &[BerechtigungsWert]) -> BerechtigungsWert {
    if werte.is_empty() {
        return BerechtigungsWert::TriState(TriState::Skip);
    }

    // Alle TriState: Deny > Grant > Skip
    let mut hat_grant = false;
    let mut hat_deny = false;
    let mut min_int_limit: Option<i64> = None;
    let mut scope_schnitt: Option<Vec<String>> = None;

    for w in werte {
        match w {
            BerechtigungsWert::TriState(ts) => match ts {
                TriState::Deny => hat_deny = true,
                TriState::Grant => hat_grant = true,
                TriState::Skip => {}
            },
            BerechtigungsWert::IntLimit(limit) => {
                min_int_limit = Some(match min_int_limit {
                    None => *limit,
                    Some(current) => current.min(*limit),
                });
            }
            BerechtigungsWert::Scope(s) => {
                scope_schnitt = Some(match scope_schnitt {
                    None => s.clone(),
                    Some(current) => {
                        // Schnittmenge berechnen (restriktiver)
                        current.into_iter().filter(|v| s.contains(v)).collect()
                    }
                });
            }
        }
    }

    if let Some(limit) = min_int_limit {
        return BerechtigungsWert::IntLimit(limit);
    }
    if let Some(scope) = scope_schnitt {
        return BerechtigungsWert::Scope(scope);
    }

    if hat_deny {
        BerechtigungsWert::TriState(TriState::Deny)
    } else if hat_grant {
        BerechtigungsWert::TriState(TriState::Grant)
    } else {
        BerechtigungsWert::TriState(TriState::Skip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant() -> BerechtigungsWert {
        BerechtigungsWert::TriState(TriState::Grant)
    }

    fn deny() -> BerechtigungsWert {
        BerechtigungsWert::TriState(TriState::Deny)
    }

    fn skip() -> BerechtigungsWert {
        BerechtigungsWert::TriState(TriState::Skip)
    }

    #[test]
    fn merge_deny_gewinnt_ueber_grant() {
        let ergebnis = merge_werte(&[grant(), deny()]);
        assert_eq!(ergebnis, deny());
    }

    #[test]
    fn merge_grant_gewinnt_ueber_skip() {
        let ergebnis = merge_werte(&[skip(), grant()]);
        assert_eq!(ergebnis, grant());
    }

    #[test]
    fn merge_nur_skip() {
        let ergebnis = merge_werte(&[skip(), skip()]);
        assert_eq!(ergebnis, skip());
    }

    #[test]
    fn merge_int_limit_minimum() {
        let ergebnis = merge_werte(&[
            BerechtigungsWert::IntLimit(100),
            BerechtigungsWert::IntLimit(50),
            BerechtigungsWert::IntLimit(75),
        ]);
        assert_eq!(ergebnis, BerechtigungsWert::IntLimit(50));
    }

    #[test]
    fn merge_scope_schnittmenge() {
        let ergebnis = merge_werte(&[
            BerechtigungsWert::Scope(vec!["a".into(), "b".into(), "c".into()]),
            BerechtigungsWert::Scope(vec!["b".into(), "c".into(), "d".into()]),
        ]);
        if let BerechtigungsWert::Scope(mut s) = ergebnis {
            s.sort();
            assert_eq!(s, vec!["b", "c"]);
        } else {
            panic!("Erwarteter Scope-Wert");
        }
    }

    #[test]
    fn aufloesen_individual_hat_prioritaet() {
        let eingabe = BerechtigungsEingabe {
            individual: vec![("can_speak".into(), grant())],
            kanal_gruppe: Some(vec![("can_speak".into(), deny())]),
            kanal_default: vec![("can_speak".into(), deny())],
            server_gruppen: vec![("admin".into(), vec![("can_speak".into(), deny())])],
            server_default: vec![("can_speak".into(), deny())],
        };

        let ergebnis = berechtigungen_aufloesen(&eingabe);
        let perm = ergebnis.get("can_speak").unwrap();
        assert_eq!(perm.wert, grant());
        assert_eq!(perm.stufe, BerechtigungsStufe::Individual);
    }

    #[test]
    fn aufloesen_skip_wird_uebersprungen() {
        let eingabe = BerechtigungsEingabe {
            individual: vec![("can_speak".into(), skip())],
            kanal_gruppe: None,
            kanal_default: vec![],
            server_gruppen: vec![],
            server_default: vec![("can_speak".into(), grant())],
        };

        let ergebnis = berechtigungen_aufloesen(&eingabe);
        // Skip auf Individual-Ebene -> Server-Default greift
        let perm = ergebnis.get("can_speak").unwrap();
        assert_eq!(perm.wert, grant());
        assert_eq!(perm.stufe, BerechtigungsStufe::ServerDefault);
    }

    #[test]
    fn aufloesen_kanal_gruppe_vor_kanal_default() {
        let eingabe = BerechtigungsEingabe {
            individual: vec![],
            kanal_gruppe: Some(vec![("can_upload".into(), deny())]),
            kanal_default: vec![("can_upload".into(), grant())],
            server_gruppen: vec![],
            server_default: vec![],
        };

        let ergebnis = berechtigungen_aufloesen(&eingabe);
        let perm = ergebnis.get("can_upload").unwrap();
        assert_eq!(perm.wert, deny());
        assert_eq!(perm.stufe, BerechtigungsStufe::KanalGruppe);
    }

    #[test]
    fn aufloesen_server_gruppen_reihenfolge() {
        let eingabe = BerechtigungsEingabe {
            individual: vec![],
            kanal_gruppe: None,
            kanal_default: vec![],
            server_gruppen: vec![
                ("admin".into(), vec![("can_ban".into(), grant())]),
                ("user".into(), vec![("can_ban".into(), deny())]),
            ],
            server_default: vec![],
        };

        let ergebnis = berechtigungen_aufloesen(&eingabe);
        // Erste Gruppe (hoechste Prioritaet) gewinnt
        let perm = ergebnis.get("can_ban").unwrap();
        assert_eq!(perm.wert, grant());
    }
}
