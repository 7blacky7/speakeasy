//! Oeffentliche Typen fuer den Chat-Service

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Nachrichtentyp
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NachrichtenTyp {
    Text,
    File,
    System,
}

/// Eine Chat-Nachricht (Domain-Typ, nicht DB-Record)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatNachricht {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub sender_id: Uuid,
    pub content: String,
    pub message_type: NachrichtenTyp,
    pub reply_to: Option<Uuid>,
    pub file_info: Option<DateeiInfo>,
    pub created_at: DateTime<Utc>,
    pub edited_at: Option<DateTime<Utc>>,
}

/// Datei-Informationen (fuer Nachrichten vom Typ 'file')
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateeiInfo {
    pub id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
}

/// Daten zum Hochladen einer Datei
#[derive(Debug)]
pub struct DateiUpload {
    pub channel_id: Uuid,
    pub uploader_id: Uuid,
    pub filename: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// Cursor-basierte Paginierung fuer die Nachrichten-History
#[derive(Debug, Clone, Default)]
pub struct HistoryAnfrage {
    pub channel_id: Uuid,
    /// Lade Nachrichten vor diesem Zeitstempel
    pub before: Option<DateTime<Utc>>,
    /// Maximale Anzahl (Default: 50)
    pub limit: Option<i64>,
}
