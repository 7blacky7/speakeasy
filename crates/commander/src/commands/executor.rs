//! Einheitlicher Befehlsausführer fuer alle drei Commander-Interfaces
//!
//! REST, TCP und gRPC nutzen alle denselben CommandExecutor.
//! Er enthaelt die gesamte Geschaeftslogik fuer alle Befehle.

use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use speakeasy_auth::{AuthService, BanService, PermissionService};
use speakeasy_db::{
    models::{
        AuditLogFilter, BerechtigungsWert, BerechtigungsZiel, KanalUpdate, NeuerBan, NeuerKanal,
        TriState,
    },
    repository::{
        AuditLogRepository, BanRepository, ChannelRepository, PermissionRepository, UserRepository,
    },
};

use crate::{
    auth::CommanderSession,
    commands::types::{
        BerechtigungsEintrag, BerechtigungsWertInput, Command, KanalInfo,
        LogEintrag, Response, ServerInfoResponse,
    },
    error::{CommanderError, CommanderResult},
};

/// Einheitlicher Befehlsausführer
///
/// Alle drei Interfaces (REST, TCP, gRPC) nutzen diese Struktur.
/// Sie haelt Referenzen auf alle benoenigten Repositories und Services.
pub struct CommandExecutor<U, C, P, B, A>
where
    U: UserRepository,
    C: ChannelRepository,
    P: PermissionRepository,
    B: BanRepository,
    A: AuditLogRepository,
{
    user_repo: Arc<U>,
    channel_repo: Arc<C>,
    permission_repo: Arc<P>,
    ban_repo: Arc<B>,
    audit_repo: Arc<A>,
    #[allow(dead_code)]
    auth_service: Arc<AuthService<U>>,
    #[allow(dead_code)]
    permission_service: Arc<PermissionService<P>>,
    #[allow(dead_code)]
    ban_service: Arc<BanService<B>>,
    /// Server-Name (aus Konfiguration)
    server_name: String,
    /// Server-Version
    server_version: String,
    /// Startzeit des Servers
    server_start: std::time::Instant,
}

impl<U, C, P, B, A> CommandExecutor<U, C, P, B, A>
where
    U: UserRepository,
    C: ChannelRepository,
    P: PermissionRepository,
    B: BanRepository,
    A: AuditLogRepository,
{
    /// Erstellt einen neuen CommandExecutor
    #[allow(clippy::too_many_arguments)]
    pub fn neu(
        user_repo: Arc<U>,
        channel_repo: Arc<C>,
        permission_repo: Arc<P>,
        ban_repo: Arc<B>,
        audit_repo: Arc<A>,
        auth_service: Arc<AuthService<U>>,
        permission_service: Arc<PermissionService<P>>,
        ban_service: Arc<BanService<B>>,
        server_name: String,
        server_version: String,
    ) -> Arc<Self> {
        Arc::new(Self {
            user_repo,
            channel_repo,
            permission_repo,
            ban_repo,
            audit_repo,
            auth_service,
            permission_service,
            ban_service,
            server_name,
            server_version,
            server_start: std::time::Instant::now(),
        })
    }

    /// Fuehrt einen Befehl im Kontext einer authentifizierten Session aus
    pub async fn ausfuehren(
        &self,
        cmd: Command,
        session: &CommanderSession,
    ) -> CommanderResult<Response> {
        match cmd {
            // --- Server ---
            Command::ServerInfo => self.server_info().await,
            Command::ServerEdit {
                name,
                willkommensnachricht,
                max_clients,
                host_nachricht,
            } => {
                self.server_bearbeiten(session, name, willkommensnachricht, max_clients, host_nachricht)
                    .await
            }
            Command::ServerStop { grund } => self.server_stoppen(session, grund).await,

            // --- Kanaele ---
            Command::KanalListe => self.kanal_liste().await,
            Command::KanalErstellen {
                name,
                parent_id,
                thema,
                passwort,
                max_clients,
                sort_order,
                permanent: _,
            } => {
                self.kanal_erstellen(session, name, parent_id, thema, passwort, max_clients, sort_order)
                    .await
            }
            Command::KanalBearbeiten {
                id,
                name,
                thema,
                max_clients,
                sort_order,
            } => self.kanal_bearbeiten(session, id, name, thema, max_clients, sort_order).await,
            Command::KanalLoeschen { id } => self.kanal_loeschen(session, id).await,

            // --- Clients ---
            Command::ClientListe => self.client_liste().await,
            Command::ClientKicken { client_id, grund } => {
                self.client_kicken(session, client_id, grund).await
            }
            Command::ClientBannen {
                client_id,
                dauer_secs,
                grund,
                ip_bannen,
            } => {
                self.client_bannen(session, client_id, dauer_secs, grund, ip_bannen)
                    .await
            }
            Command::ClientVerschieben { client_id, kanal_id } => {
                self.client_verschieben(session, client_id, kanal_id).await
            }
            Command::ClientPoken { client_id, nachricht } => {
                self.client_poken(session, client_id, nachricht).await
            }

            // --- Berechtigungen ---
            Command::BerechtigungListe { ziel, scope } => {
                self.berechtigung_liste(ziel, scope).await
            }
            Command::BerechtigungSetzen {
                ziel,
                permission,
                wert,
                scope,
            } => self.berechtigung_setzen(session, ziel, permission, wert, scope).await,
            Command::BerechtigungEntfernen {
                ziel,
                permission,
                scope,
            } => self.berechtigung_entfernen(session, ziel, permission, scope).await,

            // --- Dateien ---
            Command::DateiListe { kanal_id } => self.datei_liste(kanal_id).await,
            Command::DateiLoeschen { datei_id } => self.datei_loeschen(session, datei_id).await,

            // --- Logs ---
            Command::LogAbfragen {
                limit,
                offset,
                aktion_filter,
            } => self.log_abfragen(limit, offset, aktion_filter).await,
        }
    }

    // -----------------------------------------------------------------------
    // Server-Befehle
    // -----------------------------------------------------------------------

    async fn server_info(&self) -> CommanderResult<Response> {
        let benutzer_anzahl = self.user_repo.list(true).await?.len() as u32;
        Ok(Response::ServerInfo(ServerInfoResponse {
            name: self.server_name.clone(),
            willkommensnachricht: String::new(),
            max_clients: 100,
            aktuelle_clients: benutzer_anzahl,
            version: self.server_version.clone(),
            uptime_secs: self.server_start.elapsed().as_secs(),
        }))
    }

    async fn server_bearbeiten(
        &self,
        session: &CommanderSession,
        name: Option<String>,
        _willkommensnachricht: Option<String>,
        _max_clients: Option<u32>,
        _host_nachricht: Option<String>,
    ) -> CommanderResult<Response> {
        if !session.hat_scope("admin:server:write") {
            return Err(CommanderError::NichtAutorisiert(
                "Scope 'admin:server:write' erforderlich".into(),
            ));
        }
        self.audit_repo
            .log_event(
                Some(session.benutzer.id),
                "server.bearbeitet",
                Some("server"),
                None,
                serde_json::json!({ "name": name }),
            )
            .await?;
        Ok(Response::Ok)
    }

    async fn server_stoppen(
        &self,
        session: &CommanderSession,
        grund: Option<String>,
    ) -> CommanderResult<Response> {
        if !session.hat_scope("admin:server:stop") {
            return Err(CommanderError::NichtAutorisiert(
                "Scope 'admin:server:stop' erforderlich".into(),
            ));
        }
        tracing::warn!(
            aktor = %session.benutzer.username,
            grund = ?grund,
            "Server-Stopp angefordert"
        );
        self.audit_repo
            .log_event(
                Some(session.benutzer.id),
                "server.gestoppt",
                Some("server"),
                None,
                serde_json::json!({ "grund": grund }),
            )
            .await?;
        Ok(Response::Ok)
    }

    // -----------------------------------------------------------------------
    // Kanal-Befehle
    // -----------------------------------------------------------------------

    async fn kanal_liste(&self) -> CommanderResult<Response> {
        let kanaele = self.channel_repo.list().await?;
        let infos: Vec<KanalInfo> = kanaele
            .into_iter()
            .map(|k| KanalInfo {
                id: k.id,
                name: k.name,
                parent_id: k.parent_id,
                thema: k.topic,
                max_clients: k.max_clients,
                aktuelle_clients: 0,
                sort_order: k.sort_order,
                passwort_geschuetzt: k.password_hash.is_some(),
            })
            .collect();
        Ok(Response::KanalListe(infos))
    }

    async fn kanal_erstellen(
        &self,
        session: &CommanderSession,
        name: String,
        parent_id: Option<Uuid>,
        thema: Option<String>,
        passwort: Option<String>,
        max_clients: i64,
        sort_order: i64,
    ) -> CommanderResult<Response> {
        if name.trim().is_empty() {
            return Err(CommanderError::UngueltigeEingabe(
                "Kanalname darf nicht leer sein".into(),
            ));
        }
        let passwort_hash = passwort.map(|p| {
            // In Produktion: Argon2-Hash; hier vereinfacht
            format!("hash:{p}")
        });
        let kanal = self
            .channel_repo
            .create(NeuerKanal {
                name: &name,
                parent_id,
                topic: thema.as_deref(),
                password_hash: passwort_hash.as_deref(),
                max_clients,
                is_default: false,
                sort_order,
                channel_type: speakeasy_db::models::KanalTyp::Voice,
            })
            .await?;
        self.audit_repo
            .log_event(
                Some(session.benutzer.id),
                "kanal.erstellt",
                Some("channel"),
                Some(&kanal.id.to_string()),
                serde_json::json!({ "name": kanal.name }),
            )
            .await?;
        Ok(Response::Kanal(KanalInfo {
            id: kanal.id,
            name: kanal.name,
            parent_id: kanal.parent_id,
            thema: kanal.topic,
            max_clients: kanal.max_clients,
            aktuelle_clients: 0,
            sort_order: kanal.sort_order,
            passwort_geschuetzt: kanal.password_hash.is_some(),
        }))
    }

    async fn kanal_bearbeiten(
        &self,
        session: &CommanderSession,
        id: Uuid,
        name: Option<String>,
        thema: Option<Option<String>>,
        max_clients: Option<i64>,
        sort_order: Option<i64>,
    ) -> CommanderResult<Response> {
        let kanal = self
            .channel_repo
            .update(
                id,
                KanalUpdate {
                    name: name.clone(),
                    topic: thema,
                    max_clients,
                    sort_order,
                    ..Default::default()
                },
            )
            .await?;
        self.audit_repo
            .log_event(
                Some(session.benutzer.id),
                "kanal.bearbeitet",
                Some("channel"),
                Some(&id.to_string()),
                serde_json::json!({ "name": name }),
            )
            .await?;
        Ok(Response::Kanal(KanalInfo {
            id: kanal.id,
            name: kanal.name,
            parent_id: kanal.parent_id,
            thema: kanal.topic,
            max_clients: kanal.max_clients,
            aktuelle_clients: 0,
            sort_order: kanal.sort_order,
            passwort_geschuetzt: kanal.password_hash.is_some(),
        }))
    }

    async fn kanal_loeschen(
        &self,
        session: &CommanderSession,
        id: Uuid,
    ) -> CommanderResult<Response> {
        let geloescht = self.channel_repo.delete(id).await?;
        if !geloescht {
            return Err(CommanderError::NichtGefunden(format!(
                "Kanal {id} nicht gefunden"
            )));
        }
        self.audit_repo
            .log_event(
                Some(session.benutzer.id),
                "kanal.geloescht",
                Some("channel"),
                Some(&id.to_string()),
                serde_json::json!({}),
            )
            .await?;
        Ok(Response::Ok)
    }

    // -----------------------------------------------------------------------
    // Client-Befehle (ephemere Daten, Stub-Implementierung)
    // -----------------------------------------------------------------------

    async fn client_liste(&self) -> CommanderResult<Response> {
        // In Produktion: aus Voice-State/Presence laden
        Ok(Response::ClientListe(vec![]))
    }

    async fn client_kicken(
        &self,
        session: &CommanderSession,
        client_id: Uuid,
        grund: Option<String>,
    ) -> CommanderResult<Response> {
        // In Produktion: Signaling-Service benachrichtigen
        tracing::info!(
            aktor = %session.benutzer.username,
            client = %client_id,
            grund = ?grund,
            "Client wird gekickt"
        );
        self.audit_repo
            .log_event(
                Some(session.benutzer.id),
                "client.gekickt",
                Some("user"),
                Some(&client_id.to_string()),
                serde_json::json!({ "grund": grund }),
            )
            .await?;
        Ok(Response::Ok)
    }

    async fn client_bannen(
        &self,
        session: &CommanderSession,
        client_id: Uuid,
        dauer_secs: Option<u64>,
        grund: Option<String>,
        _ip_bannen: bool,
    ) -> CommanderResult<Response> {
        let laeuft_ab = dauer_secs.map(|d| Utc::now() + chrono::Duration::seconds(d as i64));
        self.ban_repo
            .create(NeuerBan {
                user_id: Some(client_id),
                ip: None,
                reason: grund.as_deref().unwrap_or("Kein Grund angegeben"),
                banned_by: Some(session.benutzer.id),
                expires_at: laeuft_ab,
            })
            .await?;
        self.audit_repo
            .log_event(
                Some(session.benutzer.id),
                "client.gebannt",
                Some("user"),
                Some(&client_id.to_string()),
                serde_json::json!({ "grund": grund, "dauer_secs": dauer_secs }),
            )
            .await?;
        Ok(Response::Ok)
    }

    async fn client_verschieben(
        &self,
        session: &CommanderSession,
        client_id: Uuid,
        kanal_id: Uuid,
    ) -> CommanderResult<Response> {
        // In Produktion: Signaling-Service benachrichtigen
        tracing::info!(
            aktor = %session.benutzer.username,
            client = %client_id,
            ziel_kanal = %kanal_id,
            "Client wird verschoben"
        );
        self.audit_repo
            .log_event(
                Some(session.benutzer.id),
                "client.verschoben",
                Some("user"),
                Some(&client_id.to_string()),
                serde_json::json!({ "ziel_kanal": kanal_id }),
            )
            .await?;
        Ok(Response::Ok)
    }

    async fn client_poken(
        &self,
        session: &CommanderSession,
        client_id: Uuid,
        nachricht: String,
    ) -> CommanderResult<Response> {
        if nachricht.len() > 500 {
            return Err(CommanderError::UngueltigeEingabe(
                "Nachricht zu lang (max. 500 Zeichen)".into(),
            ));
        }
        // In Produktion: Echtzeit-Benachrichtigung senden
        tracing::info!(
            aktor = %session.benutzer.username,
            client = %client_id,
            "Client wird angepikt"
        );
        self.audit_repo
            .log_event(
                Some(session.benutzer.id),
                "client.gepikt",
                Some("user"),
                Some(&client_id.to_string()),
                serde_json::json!({ "nachricht": nachricht }),
            )
            .await?;
        Ok(Response::Ok)
    }

    // -----------------------------------------------------------------------
    // Berechtigungs-Befehle
    // -----------------------------------------------------------------------

    async fn berechtigung_liste(
        &self,
        ziel: String,
        scope: String,
    ) -> CommanderResult<Response> {
        let (ziel_parsed, kanal_id) = ziel_parsen(&ziel, &scope)?;
        let eintraege = self
            .permission_repo
            .get_permissions(&ziel_parsed, kanal_id)
            .await?;
        let result: Vec<BerechtigungsEintrag> = eintraege
            .into_iter()
            .map(|(key, wert)| BerechtigungsEintrag {
                permission: key,
                wert: db_wert_zu_input(wert),
            })
            .collect();
        Ok(Response::BerechtigungListe(result))
    }

    async fn berechtigung_setzen(
        &self,
        session: &CommanderSession,
        ziel: String,
        permission: String,
        wert: BerechtigungsWertInput,
        scope: String,
    ) -> CommanderResult<Response> {
        let (ziel_parsed, kanal_id) = ziel_parsen(&ziel, &scope)?;
        let db_wert = input_zu_db_wert(wert);
        self.permission_repo
            .set_permission(&ziel_parsed, &permission, db_wert, kanal_id)
            .await?;
        self.audit_repo
            .log_event(
                Some(session.benutzer.id),
                "berechtigung.gesetzt",
                Some("permission"),
                Some(&permission),
                serde_json::json!({ "ziel": ziel, "scope": scope }),
            )
            .await?;
        Ok(Response::Ok)
    }

    async fn berechtigung_entfernen(
        &self,
        session: &CommanderSession,
        ziel: String,
        permission: String,
        scope: String,
    ) -> CommanderResult<Response> {
        let (ziel_parsed, kanal_id) = ziel_parsen(&ziel, &scope)?;
        self.permission_repo
            .remove_permission(&ziel_parsed, &permission, kanal_id)
            .await?;
        self.audit_repo
            .log_event(
                Some(session.benutzer.id),
                "berechtigung.entfernt",
                Some("permission"),
                Some(&permission),
                serde_json::json!({ "ziel": ziel, "scope": scope }),
            )
            .await?;
        Ok(Response::Ok)
    }

    // -----------------------------------------------------------------------
    // Datei-Befehle (Stub)
    // -----------------------------------------------------------------------

    async fn datei_liste(&self, _kanal_id: Uuid) -> CommanderResult<Response> {
        // In Produktion: Datei-Repository abfragen
        Ok(Response::DateiListe(vec![]))
    }

    async fn datei_loeschen(
        &self,
        session: &CommanderSession,
        datei_id: String,
    ) -> CommanderResult<Response> {
        // In Produktion: Datei-Repository und Storage-Backend
        tracing::info!(
            aktor = %session.benutzer.username,
            datei = %datei_id,
            "Datei wird geloescht"
        );
        self.audit_repo
            .log_event(
                Some(session.benutzer.id),
                "datei.geloescht",
                Some("file"),
                Some(&datei_id),
                serde_json::json!({}),
            )
            .await?;
        Ok(Response::Ok)
    }

    // -----------------------------------------------------------------------
    // Log-Befehle
    // -----------------------------------------------------------------------

    async fn log_abfragen(
        &self,
        limit: u32,
        offset: u32,
        aktion_filter: Option<String>,
    ) -> CommanderResult<Response> {
        let filter = AuditLogFilter {
            action: aktion_filter,
            limit: Some(limit as i64),
            offset: Some(offset as i64),
            ..Default::default()
        };
        let ereignisse = self.audit_repo.list_events(filter).await?;
        let eintraege: Vec<LogEintrag> = ereignisse
            .into_iter()
            .map(|e| LogEintrag {
                id: e.id,
                aktor_id: e.actor_id,
                aktion: e.action,
                ziel_typ: e.target_type,
                ziel_id: e.target_id,
                zeitstempel: e.timestamp,
                details: e.details,
            })
            .collect();
        Ok(Response::LogEintraege(eintraege))
    }
}

// ---------------------------------------------------------------------------
// Hilfsfunktionen
// ---------------------------------------------------------------------------

/// Parst ein Ziel-String ("user:<uuid>", "server_group:<uuid>", "server_default")
/// und einen Scope-String ("server" oder "channel:<uuid>") in DB-Typen.
fn ziel_parsen(
    ziel: &str,
    scope: &str,
) -> CommanderResult<(BerechtigungsZiel, Option<Uuid>)> {
    let ziel_parsed = if ziel == "server_default" {
        BerechtigungsZiel::ServerDefault
    } else {
        let (typ, id_str) = ziel.split_once(':').ok_or_else(|| {
            CommanderError::UngueltigeEingabe(format!("Ungueltiges Ziel-Format: {ziel}"))
        })?;
        let id = Uuid::parse_str(id_str).map_err(|_| {
            CommanderError::UngueltigeEingabe(format!("Ungueltige UUID: {id_str}"))
        })?;
        match typ {
            "user" => BerechtigungsZiel::Benutzer(id),
            "server_group" => BerechtigungsZiel::ServerGruppe(id),
            "channel_group" => BerechtigungsZiel::KanalGruppe(id),
            other => {
                return Err(CommanderError::UngueltigeEingabe(format!(
                    "Unbekannter Ziel-Typ: {other}"
                )))
            }
        }
    };

    let kanal_id = if scope == "server" {
        None
    } else if let Some(id_str) = scope.strip_prefix("channel:") {
        Some(Uuid::parse_str(id_str).map_err(|_| {
            CommanderError::UngueltigeEingabe(format!("Ungueltige Kanal-UUID: {id_str}"))
        })?)
    } else {
        return Err(CommanderError::UngueltigeEingabe(format!(
            "Ungueltiger Scope: {scope}"
        )));
    };

    Ok((ziel_parsed, kanal_id))
}

fn db_wert_zu_input(wert: BerechtigungsWert) -> BerechtigungsWertInput {
    match wert {
        BerechtigungsWert::TriState(speakeasy_db::models::TriState::Grant) => {
            BerechtigungsWertInput::Grant
        }
        BerechtigungsWert::TriState(speakeasy_db::models::TriState::Deny) => {
            BerechtigungsWertInput::Deny
        }
        BerechtigungsWert::TriState(speakeasy_db::models::TriState::Skip) => {
            BerechtigungsWertInput::Skip
        }
        BerechtigungsWert::IntLimit(n) => BerechtigungsWertInput::IntLimit(n),
        BerechtigungsWert::Scope(_) => BerechtigungsWertInput::Skip,
    }
}

fn input_zu_db_wert(wert: BerechtigungsWertInput) -> BerechtigungsWert {
    match wert {
        BerechtigungsWertInput::Grant => BerechtigungsWert::TriState(TriState::Grant),
        BerechtigungsWertInput::Deny => BerechtigungsWert::TriState(TriState::Deny),
        BerechtigungsWertInput::Skip => BerechtigungsWert::TriState(TriState::Skip),
        BerechtigungsWertInput::IntLimit(n) => BerechtigungsWert::IntLimit(n),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ziel_parsen_server_default() {
        let (ziel, kanal) = ziel_parsen("server_default", "server").unwrap();
        assert_eq!(ziel, BerechtigungsZiel::ServerDefault);
        assert!(kanal.is_none());
    }

    #[test]
    fn ziel_parsen_benutzer() {
        let id = Uuid::new_v4();
        let (ziel, _) = ziel_parsen(&format!("user:{id}"), "server").unwrap();
        assert_eq!(ziel, BerechtigungsZiel::Benutzer(id));
    }

    #[test]
    fn ziel_parsen_kanal_scope() {
        let kanal_id = Uuid::new_v4();
        let gruppe_id = Uuid::new_v4();
        let (_, kanal) =
            ziel_parsen(&format!("server_group:{gruppe_id}"), &format!("channel:{kanal_id}"))
                .unwrap();
        assert_eq!(kanal, Some(kanal_id));
    }

    #[test]
    fn ziel_parsen_ungueltig() {
        let ergebnis = ziel_parsen("unbekannt", "server");
        assert!(ergebnis.is_err());
    }

    #[test]
    fn ziel_parsen_ungueltige_uuid() {
        let ergebnis = ziel_parsen("user:keine-uuid", "server");
        assert!(ergebnis.is_err());
    }

    #[test]
    fn db_wert_konvertierung_grant() {
        let input = BerechtigungsWertInput::Grant;
        let db = input_zu_db_wert(input.clone());
        let zurueck = db_wert_zu_input(db);
        assert_eq!(zurueck, input);
    }

    #[test]
    fn db_wert_konvertierung_int_limit() {
        let input = BerechtigungsWertInput::IntLimit(42);
        let db = input_zu_db_wert(input.clone());
        let zurueck = db_wert_zu_input(db);
        assert_eq!(zurueck, input);
    }
}
