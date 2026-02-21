//! ChatService – Nachrichten senden, empfangen, editieren, loeschen

use std::sync::Arc;

use uuid::Uuid;

use speakeasy_db::{
    models::{NachrichtenFilter, NachrichtenTyp as DbNachrichtenTyp, NeueNachricht},
    ChatMessageRepository,
};

use crate::{
    error::{ChatError, ChatResult},
    types::{ChatNachricht, HistoryAnfrage, NachrichtenTyp},
};

/// ChatService verwaltet Text-Nachrichten in Kanaelen
pub struct ChatService<R: ChatMessageRepository> {
    repo: Arc<R>,
}

impl<R: ChatMessageRepository> ChatService<R> {
    /// Erstellt einen neuen ChatService
    pub fn neu(repo: Arc<R>) -> Arc<Self> {
        Arc::new(Self { repo })
    }

    /// Nachricht in einem Kanal senden
    pub async fn nachricht_senden(
        &self,
        channel_id: Uuid,
        sender_id: Uuid,
        content: &str,
        reply_to: Option<Uuid>,
    ) -> ChatResult<ChatNachricht> {
        if content.trim().is_empty() {
            return Err(ChatError::UngueltigeEingabe(
                "Nachrichteninhalt darf nicht leer sein".into(),
            ));
        }

        if content.len() > 4096 {
            return Err(ChatError::UngueltigeEingabe(format!(
                "Nachricht zu lang: {} Zeichen (Maximum: 4096)",
                content.len()
            )));
        }

        let record = self
            .repo
            .create(NeueNachricht {
                channel_id,
                sender_id,
                content,
                message_type: DbNachrichtenTyp::Text,
                reply_to,
            })
            .await?;

        Ok(record_to_nachricht(record, None))
    }

    /// Nachricht editieren (nur eigene Nachrichten)
    pub async fn nachricht_editieren(
        &self,
        message_id: Uuid,
        sender_id: Uuid,
        new_content: &str,
    ) -> ChatResult<ChatNachricht> {
        if new_content.trim().is_empty() {
            return Err(ChatError::UngueltigeEingabe(
                "Nachrichteninhalt darf nicht leer sein".into(),
            ));
        }

        if new_content.len() > 4096 {
            return Err(ChatError::UngueltigeEingabe(format!(
                "Nachricht zu lang: {} Zeichen (Maximum: 4096)",
                new_content.len()
            )));
        }

        // Nachricht laden und Berechtigung pruefen
        let existing = self
            .repo
            .get_by_id(message_id)
            .await?
            .ok_or_else(|| ChatError::NachrichtNichtGefunden(message_id.to_string()))?;

        if existing.sender_id != sender_id {
            return Err(ChatError::KeineBerechtigung(
                "Nur der Verfasser kann die Nachricht editieren".into(),
            ));
        }

        if existing.deleted_at.is_some() {
            return Err(ChatError::NachrichtNichtGefunden(message_id.to_string()));
        }

        let record = self.repo.update_content(message_id, new_content).await?;
        Ok(record_to_nachricht(record, None))
    }

    /// Nachricht weich loeschen (Soft-Delete)
    pub async fn nachricht_loeschen(&self, message_id: Uuid, requester_id: Uuid) -> ChatResult<()> {
        let existing = self
            .repo
            .get_by_id(message_id)
            .await?
            .ok_or_else(|| ChatError::NachrichtNichtGefunden(message_id.to_string()))?;

        if existing.sender_id != requester_id {
            return Err(ChatError::KeineBerechtigung(
                "Nur der Verfasser kann die Nachricht loeschen".into(),
            ));
        }

        let geloescht = self.repo.soft_delete(message_id).await?;
        if !geloescht {
            return Err(ChatError::NachrichtNichtGefunden(message_id.to_string()));
        }

        Ok(())
    }

    /// Nachrichten-History eines Kanals laden (Cursor-Pagination)
    pub async fn history_laden(&self, anfrage: HistoryAnfrage) -> ChatResult<Vec<ChatNachricht>> {
        let records = self
            .repo
            .get_history(NachrichtenFilter {
                channel_id: anfrage.channel_id,
                before: anfrage.before,
                limit: anfrage.limit,
            })
            .await?;

        Ok(records
            .into_iter()
            .map(|r| record_to_nachricht(r, None))
            .collect())
    }

    /// Nachrichten eines Kanals durchsuchen
    pub async fn nachrichten_suchen(
        &self,
        channel_id: Uuid,
        query: &str,
    ) -> ChatResult<Vec<ChatNachricht>> {
        if query.trim().is_empty() {
            return Err(ChatError::UngueltigeEingabe(
                "Suchbegriff darf nicht leer sein".into(),
            ));
        }

        let records = self.repo.search(channel_id, query, 50).await?;
        Ok(records
            .into_iter()
            .map(|r| record_to_nachricht(r, None))
            .collect())
    }
}

/// Konvertiert einen DB-Record in den Domain-Typ
fn record_to_nachricht(
    record: speakeasy_db::models::ChatNachrichtRecord,
    file_info: Option<crate::types::DateeiInfo>,
) -> ChatNachricht {
    let message_type = match record.message_type {
        DbNachrichtenTyp::Text => NachrichtenTyp::Text,
        DbNachrichtenTyp::File => NachrichtenTyp::File,
        DbNachrichtenTyp::System => NachrichtenTyp::System,
    };

    ChatNachricht {
        id: record.id,
        channel_id: record.channel_id,
        sender_id: record.sender_id,
        content: record.content,
        message_type,
        reply_to: record.reply_to,
        file_info,
        created_at: record.created_at,
        edited_at: record.edited_at,
    }
}
