//! Unit-Tests fuer den FileService

use std::sync::Arc;

use speakeasy_db::models::{KanalTyp, NeuerBenutzer, NeuerKanal};
use speakeasy_db::{ChannelRepository, SqliteDb, UserRepository};
use uuid::Uuid;

use crate::{
    error::ChatError, file_service::FileService, storage::DiskStorage, types::DateiUpload,
};

async fn test_db() -> Arc<SqliteDb> {
    Arc::new(
        SqliteDb::in_memory()
            .await
            .expect("In-Memory-DB konnte nicht geoeffnet werden"),
    )
}

async fn setup(db: &Arc<SqliteDb>) -> (Uuid, Uuid) {
    let user = UserRepository::create(
        db.as_ref(),
        NeuerBenutzer {
            username: "uploader",
            password_hash: "hash",
        },
    )
    .await
    .expect("User anlegen fehlgeschlagen");

    let kanal = ChannelRepository::create(
        db.as_ref(),
        NeuerKanal {
            name: "dateikanal",
            channel_type: KanalTyp::Text,
            ..Default::default()
        },
    )
    .await
    .expect("Kanal anlegen fehlgeschlagen");

    (kanal.id, user.id)
}

fn temp_storage() -> (DiskStorage, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("Temp-Verzeichnis konnte nicht erstellt werden");
    let storage = DiskStorage::new(dir.path());
    (storage, dir)
}

#[tokio::test]
async fn test_datei_hochladen_erfolgreich() {
    let db = test_db().await;
    let (channel_id, uploader_id) = setup(&db).await;
    let (storage, _dir) = temp_storage();
    let storage = Arc::new(storage);
    let service = FileService::neu(db.clone(), db.clone(), storage);

    let data = b"Hallo Dateiinhalt!".to_vec();
    let (info, nachricht) = service
        .datei_hochladen(
            DateiUpload {
                channel_id,
                uploader_id,
                filename: "test.txt".to_string(),
                mime_type: "text/plain".to_string(),
                data,
            },
            None,
        )
        .await
        .expect("Datei hochladen fehlgeschlagen");

    assert_eq!(info.filename, "test.txt");
    assert_eq!(info.mime_type, "text/plain");
    assert_eq!(info.size_bytes, 18);
    assert_eq!(nachricht.channel_id, channel_id);
    assert_eq!(nachricht.sender_id, uploader_id);
}

#[tokio::test]
async fn test_datei_herunterladen() {
    let db = test_db().await;
    let (channel_id, uploader_id) = setup(&db).await;
    let (storage, _dir) = temp_storage();
    let storage = Arc::new(storage);
    let service = FileService::neu(db.clone(), db.clone(), storage);

    let original_data = b"Download-Test".to_vec();
    let (info, _) = service
        .datei_hochladen(
            DateiUpload {
                channel_id,
                uploader_id,
                filename: "download.txt".to_string(),
                mime_type: "text/plain".to_string(),
                data: original_data.clone(),
            },
            None,
        )
        .await
        .unwrap();

    let (dl_info, dl_data) = service
        .datei_herunterladen(info.id)
        .await
        .expect("Datei herunterladen fehlgeschlagen");

    assert_eq!(dl_info.id, info.id);
    assert_eq!(dl_data, original_data);
}

#[tokio::test]
async fn test_datei_loeschen() {
    let db = test_db().await;
    let (channel_id, uploader_id) = setup(&db).await;
    let (storage, _dir) = temp_storage();
    let storage = Arc::new(storage);
    let service = FileService::neu(db.clone(), db.clone(), storage);

    let (info, _) = service
        .datei_hochladen(
            DateiUpload {
                channel_id,
                uploader_id,
                filename: "zu_loeschen.txt".to_string(),
                mime_type: "text/plain".to_string(),
                data: b"Inhalt".to_vec(),
            },
            None,
        )
        .await
        .unwrap();

    service
        .datei_loeschen(info.id, uploader_id, None)
        .await
        .expect("Datei loeschen fehlgeschlagen");

    // Nach dem Loeschen sollte die Datei nicht mehr abrufbar sein
    let result = service.datei_herunterladen(info.id).await;
    assert!(matches!(result, Err(ChatError::DateiNichtGefunden(_))));
}

#[tokio::test]
async fn test_fremde_datei_nicht_loeschbar() {
    let db = test_db().await;
    let (channel_id, uploader_id) = setup(&db).await;
    let (storage, _dir) = temp_storage();
    let storage = Arc::new(storage);
    let service = FileService::neu(db.clone(), db.clone(), storage);

    let (info, _) = service
        .datei_hochladen(
            DateiUpload {
                channel_id,
                uploader_id,
                filename: "gehuetet.txt".to_string(),
                mime_type: "text/plain".to_string(),
                data: b"Geheimnis".to_vec(),
            },
            None,
        )
        .await
        .unwrap();

    let fremder_user = UserRepository::create(
        db.as_ref(),
        NeuerBenutzer {
            username: "fremder",
            password_hash: "hash2",
        },
    )
    .await
    .unwrap();

    let result = service.datei_loeschen(info.id, fremder_user.id, None).await;

    assert!(matches!(result, Err(ChatError::KeineBerechtigung(_))));
}

#[tokio::test]
async fn test_quota_max_dateigroesse() {
    let db = test_db().await;
    let (channel_id, uploader_id) = setup(&db).await;
    let (storage, _dir) = temp_storage();
    let storage = Arc::new(storage);
    let service = FileService::neu(db.clone(), db.clone(), storage);

    // Eine 10MB + 1 Byte grosse Datei (ueberschreitet Standard-Limit)
    let zu_grosse_datei = vec![0u8; 10 * 1024 * 1024 + 1];
    let result = service
        .datei_hochladen(
            DateiUpload {
                channel_id,
                uploader_id,
                filename: "riesig.bin".to_string(),
                mime_type: "application/octet-stream".to_string(),
                data: zu_grosse_datei,
            },
            None,
        )
        .await;

    assert!(matches!(result, Err(ChatError::DateiZuGross { .. })));
}

#[tokio::test]
async fn test_leerer_dateiname_abgelehnt() {
    let db = test_db().await;
    let (channel_id, uploader_id) = setup(&db).await;
    let (storage, _dir) = temp_storage();
    let storage = Arc::new(storage);
    let service = FileService::neu(db.clone(), db.clone(), storage);

    let result = service
        .datei_hochladen(
            DateiUpload {
                channel_id,
                uploader_id,
                filename: "   ".to_string(),
                mime_type: "text/plain".to_string(),
                data: b"inhalt".to_vec(),
            },
            None,
        )
        .await;

    assert!(matches!(result, Err(ChatError::UngueltigeEingabe(_))));
}

#[tokio::test]
async fn test_dateien_auflisten() {
    let db = test_db().await;
    let (channel_id, uploader_id) = setup(&db).await;
    let (storage, _dir) = temp_storage();
    let storage = Arc::new(storage);
    let service = FileService::neu(db.clone(), db.clone(), storage);

    for i in 1..=3 {
        service
            .datei_hochladen(
                DateiUpload {
                    channel_id,
                    uploader_id,
                    filename: format!("datei{i}.txt"),
                    mime_type: "text/plain".to_string(),
                    data: format!("Inhalt {i}").into_bytes(),
                },
                None,
            )
            .await
            .unwrap();
    }

    let liste = service
        .dateien_auflisten(channel_id)
        .await
        .expect("Dateien auflisten fehlgeschlagen");

    assert_eq!(liste.len(), 3);
}

#[tokio::test]
async fn test_nicht_vorhandene_datei_download() {
    let db = test_db().await;
    let (_, _) = setup(&db).await;
    let (storage, _dir) = temp_storage();
    let storage = Arc::new(storage);
    let service = FileService::neu(db.clone(), db.clone(), storage);

    let result = service.datei_herunterladen(Uuid::new_v4()).await;
    assert!(matches!(result, Err(ChatError::DateiNichtGefunden(_))));
}
