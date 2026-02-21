//! Unit-Tests fuer den ChatService

use std::sync::Arc;

use speakeasy_db::SqliteDb;
use speakeasy_db::ChannelRepository;
use speakeasy_db::UserRepository;
use speakeasy_db::models::{NeuerBenutzer, NeuerKanal, KanalTyp};
use uuid::Uuid;
#[allow(unused_imports)]
use chrono;

use crate::{
    service::ChatService,
    types::HistoryAnfrage,
    error::ChatError,
};

async fn test_db() -> Arc<SqliteDb> {
    Arc::new(SqliteDb::in_memory().await.expect("In-Memory-DB konnte nicht geoeffnet werden"))
}

async fn setup_kanal_und_user(db: &Arc<SqliteDb>) -> (Uuid, Uuid) {
    let user = UserRepository::create(db.as_ref(), NeuerBenutzer {
        username: "testuser",
        password_hash: "hash",
    }).await.expect("User anlegen fehlgeschlagen");

    let kanal = ChannelRepository::create(db.as_ref(), NeuerKanal {
        name: "testkanal",
        channel_type: KanalTyp::Text,
        ..Default::default()
    }).await.expect("Kanal anlegen fehlgeschlagen");

    (kanal.id, user.id)
}

#[tokio::test]
async fn test_nachricht_senden_erfolgreich() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db);

    let nachricht = service
        .nachricht_senden(channel_id, sender_id, "Hallo Welt!", None)
        .await
        .expect("Nachricht senden fehlgeschlagen");

    assert_eq!(nachricht.content, "Hallo Welt!");
    assert_eq!(nachricht.channel_id, channel_id);
    assert_eq!(nachricht.sender_id, sender_id);
    assert!(nachricht.reply_to.is_none());
    assert!(nachricht.edited_at.is_none());
}

#[tokio::test]
async fn test_nachricht_mit_antwort_senden() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db);

    let original = service
        .nachricht_senden(channel_id, sender_id, "Originalnachricht", None)
        .await
        .expect("Originalnachricht senden fehlgeschlagen");

    let antwort = service
        .nachricht_senden(channel_id, sender_id, "Antwort!", Some(original.id))
        .await
        .expect("Antwort senden fehlgeschlagen");

    assert_eq!(antwort.reply_to, Some(original.id));
}

#[tokio::test]
async fn test_leere_nachricht_abgelehnt() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db);

    let result = service
        .nachricht_senden(channel_id, sender_id, "   ", None)
        .await;

    assert!(matches!(result, Err(ChatError::UngueltigeEingabe(_))));
}

#[tokio::test]
async fn test_zu_lange_nachricht_abgelehnt() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db);

    let zu_lang = "x".repeat(4097);
    let result = service
        .nachricht_senden(channel_id, sender_id, &zu_lang, None)
        .await;

    assert!(matches!(result, Err(ChatError::UngueltigeEingabe(_))));
}

#[tokio::test]
async fn test_nachricht_editieren() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db);

    let nachricht = service
        .nachricht_senden(channel_id, sender_id, "Original", None)
        .await
        .expect("Nachricht senden fehlgeschlagen");

    let editiert = service
        .nachricht_editieren(nachricht.id, sender_id, "Editiert")
        .await
        .expect("Nachricht editieren fehlgeschlagen");

    assert_eq!(editiert.content, "Editiert");
    assert!(editiert.edited_at.is_some());
}

#[tokio::test]
async fn test_fremde_nachricht_nicht_editierbar() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db.clone());

    let nachricht = service
        .nachricht_senden(channel_id, sender_id, "Original", None)
        .await
        .expect("Nachricht senden fehlgeschlagen");

    let anderer_user = UserRepository::create(db.as_ref(), NeuerBenutzer {
        username: "anderer_user",
        password_hash: "hash2",
    }).await.expect("User anlegen fehlgeschlagen");

    let result = service
        .nachricht_editieren(nachricht.id, anderer_user.id, "Nicht erlaubt")
        .await;

    assert!(matches!(result, Err(ChatError::KeineBerechtigung(_))));
}

#[tokio::test]
async fn test_nachricht_loeschen() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db);

    let nachricht = service
        .nachricht_senden(channel_id, sender_id, "Zu loeschen", None)
        .await
        .expect("Nachricht senden fehlgeschlagen");

    service
        .nachricht_loeschen(nachricht.id, sender_id)
        .await
        .expect("Nachricht loeschen fehlgeschlagen");
}

#[tokio::test]
async fn test_fremde_nachricht_nicht_loeschbar() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db.clone());

    let nachricht = service
        .nachricht_senden(channel_id, sender_id, "Zu loeschen", None)
        .await
        .expect("Nachricht senden fehlgeschlagen");

    let anderer_user = UserRepository::create(db.as_ref(), NeuerBenutzer {
        username: "fremder_user",
        password_hash: "hash3",
    }).await.expect("User anlegen fehlgeschlagen");

    let result = service
        .nachricht_loeschen(nachricht.id, anderer_user.id)
        .await;

    assert!(matches!(result, Err(ChatError::KeineBerechtigung(_))));
}

#[tokio::test]
async fn test_history_laden() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db);

    for i in 1..=5 {
        service
            .nachricht_senden(channel_id, sender_id, &format!("Nachricht {i}"), None)
            .await
            .expect("Nachricht senden fehlgeschlagen");
    }

    let history = service
        .history_laden(HistoryAnfrage {
            channel_id,
            before: None,
            limit: Some(10),
        })
        .await
        .expect("History laden fehlgeschlagen");

    assert_eq!(history.len(), 5);
    // Chronologisch sortiert (aelteste zuerst)
    assert_eq!(history[0].content, "Nachricht 1");
    assert_eq!(history[4].content, "Nachricht 5");
}

#[tokio::test]
async fn test_history_paginierung() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db);

    for i in 1..=10 {
        service
            .nachricht_senden(channel_id, sender_id, &format!("Nachricht {i}"), None)
            .await
            .expect("Nachricht senden fehlgeschlagen");
    }

    // Nur 3 Nachrichten laden
    let seite = service
        .history_laden(HistoryAnfrage {
            channel_id,
            before: None,
            limit: Some(3),
        })
        .await
        .expect("History laden fehlgeschlagen");

    // Limit wird eingehalten
    assert_eq!(seite.len(), 3);
}

#[tokio::test]
async fn test_history_before_cursor() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db);

    // Erste Nachricht senden, Timestamp merken
    let erste = service
        .nachricht_senden(channel_id, sender_id, "Erste", None)
        .await
        .unwrap();

    // Danach weitere Nachrichten senden
    for i in 2..=5 {
        service
            .nachricht_senden(channel_id, sender_id, &format!("Nachricht {i}"), None)
            .await
            .unwrap();
    }

    // Nachrichten VOR einem Zeitpunkt nach der ersten Nachricht laden
    let nach_erster = erste.created_at + chrono::Duration::seconds(1);
    let history = service
        .history_laden(HistoryAnfrage {
            channel_id,
            before: Some(nach_erster),
            limit: Some(50),
        })
        .await
        .unwrap();

    // Nur Nachrichten die vor dem Cursor liegen
    assert!(history.iter().all(|n| n.created_at < nach_erster));
}

#[tokio::test]
async fn test_nachrichten_suchen() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db);

    service.nachricht_senden(channel_id, sender_id, "Guten Morgen!", None).await.unwrap();
    service.nachricht_senden(channel_id, sender_id, "Guten Abend!", None).await.unwrap();
    service.nachricht_senden(channel_id, sender_id, "Tschuess!", None).await.unwrap();

    let ergebnisse = service
        .nachrichten_suchen(channel_id, "Guten")
        .await
        .expect("Suche fehlgeschlagen");

    assert_eq!(ergebnisse.len(), 2);
}

#[tokio::test]
async fn test_suche_leerer_query_abgelehnt() {
    let db = test_db().await;
    let (channel_id, _) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db);

    let result = service.nachrichten_suchen(channel_id, "  ").await;
    assert!(matches!(result, Err(ChatError::UngueltigeEingabe(_))));
}

#[tokio::test]
async fn test_geloeschte_nachrichten_nicht_in_history() {
    let db = test_db().await;
    let (channel_id, sender_id) = setup_kanal_und_user(&db).await;
    let service = ChatService::neu(db);

    let n1 = service.nachricht_senden(channel_id, sender_id, "Bleibt", None).await.unwrap();
    let n2 = service.nachricht_senden(channel_id, sender_id, "Wird geloescht", None).await.unwrap();

    service.nachricht_loeschen(n2.id, sender_id).await.unwrap();

    let history = service
        .history_laden(HistoryAnfrage { channel_id, before: None, limit: None })
        .await
        .unwrap();

    assert_eq!(history.len(), 1);
    assert_eq!(history[0].id, n1.id);
}
