//! Unit-Tests fuer das DiskStorage-Backend

use crate::storage::{DiskStorage, StorageBackend};

fn temp_storage() -> (DiskStorage, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("Temp-Verzeichnis konnte nicht erstellt werden");
    let storage = DiskStorage::new(dir.path());
    (storage, dir)
}

#[tokio::test]
async fn test_store_und_retrieve() {
    let (storage, _dir) = temp_storage();

    let data = b"Test-Dateiinhalt";
    storage
        .store("test/datei.txt", data)
        .await
        .expect("Speichern fehlgeschlagen");

    let gelesen = storage
        .retrieve("test/datei.txt")
        .await
        .expect("Lesen fehlgeschlagen");

    assert_eq!(gelesen, data);
}

#[tokio::test]
async fn test_store_erstellt_verzeichnis() {
    let (storage, dir) = temp_storage();

    storage
        .store("unterverzeichnis/noch_tiefer/datei.bin", b"daten")
        .await
        .expect("Speichern mit tiefem Pfad fehlgeschlagen");

    assert!(dir
        .path()
        .join("unterverzeichnis/noch_tiefer/datei.bin")
        .exists());
}

#[tokio::test]
async fn test_delete_entfernt_datei() {
    let (storage, dir) = temp_storage();

    storage.store("zu_loeschen.txt", b"inhalt").await.unwrap();

    assert!(dir.path().join("zu_loeschen.txt").exists());

    storage
        .delete("zu_loeschen.txt")
        .await
        .expect("Loeschen fehlgeschlagen");

    assert!(!dir.path().join("zu_loeschen.txt").exists());
}

#[tokio::test]
async fn test_delete_nicht_vorhandene_datei_kein_fehler() {
    let (storage, _dir) = temp_storage();

    // Loeschen einer nicht vorhandenen Datei sollte keinen Fehler werfen
    let result = storage.delete("existiert_nicht.txt").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_retrieve_nicht_vorhandene_datei_fehler() {
    let (storage, _dir) = temp_storage();

    let result = storage.retrieve("existiert_nicht.txt").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_store_ueberschreibt_vorhandene_datei() {
    let (storage, _dir) = temp_storage();

    storage.store("datei.txt", b"original").await.unwrap();
    storage.store("datei.txt", b"ueberschrieben").await.unwrap();

    let gelesen = storage.retrieve("datei.txt").await.unwrap();
    assert_eq!(gelesen, b"ueberschrieben");
}

#[tokio::test]
async fn test_binaere_daten_intakt() {
    let (storage, _dir) = temp_storage();

    let binaerdaten: Vec<u8> = (0u8..=255).collect();
    storage.store("binaer.bin", &binaerdaten).await.unwrap();

    let gelesen = storage.retrieve("binaer.bin").await.unwrap();
    assert_eq!(gelesen, binaerdaten);
}

#[tokio::test]
async fn test_grosse_datei() {
    let (storage, _dir) = temp_storage();

    let grosse_datei = vec![42u8; 5 * 1024 * 1024]; // 5 MB
    storage
        .store("gross.bin", &grosse_datei)
        .await
        .expect("Grosse Datei speichern fehlgeschlagen");

    let gelesen = storage.retrieve("gross.bin").await.unwrap();
    assert_eq!(gelesen.len(), grosse_datei.len());
    assert_eq!(gelesen, grosse_datei);
}
