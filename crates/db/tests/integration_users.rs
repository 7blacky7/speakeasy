//! Integration-Tests fuer UserRepository (In-Memory SQLite)

use speakeasy_db::{
    models::{BenutzerUpdate, NeuerBenutzer},
    SqliteDb, UserRepository,
};

async fn db() -> SqliteDb {
    SqliteDb::in_memory()
        .await
        .expect("In-Memory DB konnte nicht erstellt werden")
}

#[tokio::test]
async fn benutzer_erstellen_und_laden() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "alice",
            password_hash: "hash_alice",
        },
    )
    .await
    .expect("Benutzer erstellen fehlgeschlagen");

    assert_eq!(user.username, "alice");
    assert!(user.is_active);

    let geladen = UserRepository::get_by_id(&db, user.id)
        .await
        .expect("get_by_id fehlgeschlagen")
        .expect("Benutzer sollte gefunden werden");

    assert_eq!(geladen.id, user.id);
    assert_eq!(geladen.username, "alice");
}

#[tokio::test]
async fn benutzer_nach_name_laden() {
    let db = db().await;

    UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "bob",
            password_hash: "hash_bob",
        },
    )
    .await
    .unwrap();

    let gefunden = UserRepository::get_by_name(&db, "bob")
        .await
        .unwrap()
        .expect("Benutzer 'bob' sollte gefunden werden");

    assert_eq!(gefunden.username, "bob");

    let nicht_gefunden = UserRepository::get_by_name(&db, "unbekannt").await.unwrap();
    assert!(nicht_gefunden.is_none());
}

#[tokio::test]
async fn benutzer_username_unique() {
    let db = db().await;

    UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "charlie",
            password_hash: "hash1",
        },
    )
    .await
    .unwrap();

    let err = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "charlie",
            password_hash: "hash2",
        },
    )
    .await;

    assert!(err.is_err());
    assert!(err.unwrap_err().ist_eindeutigkeit());
}

#[tokio::test]
async fn benutzer_aktualisieren() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "dave",
            password_hash: "alt_hash",
        },
    )
    .await
    .unwrap();

    let aktualisiert = UserRepository::update(
        &db,
        user.id,
        BenutzerUpdate {
            password_hash: Some("neues_hash".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(aktualisiert.password_hash, "neues_hash");
    assert_eq!(aktualisiert.username, "dave");
}

#[tokio::test]
async fn benutzer_loeschen_weich() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "eve",
            password_hash: "hash_eve",
        },
    )
    .await
    .unwrap();

    let geloescht = UserRepository::delete(&db, user.id).await.unwrap();
    assert!(geloescht);

    // Benutzer ist noch vorhanden, aber inaktiv
    let geladen = UserRepository::get_by_id(&db, user.id)
        .await
        .unwrap()
        .unwrap();
    assert!(!geladen.is_active);

    // Nur aktive Benutzer auflisten
    let aktive = UserRepository::list(&db, true).await.unwrap();
    assert!(aktive.iter().all(|u| u.is_active));
}

#[tokio::test]
async fn benutzer_authentifizieren() {
    let db = db().await;

    UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "frank",
            password_hash: "korrekt_hash",
        },
    )
    .await
    .unwrap();

    // Korrekte Credentials
    let auth = UserRepository::authenticate(&db, "frank", "korrekt_hash")
        .await
        .unwrap();
    assert!(auth.is_some());

    // Falsches Passwort
    let falsch = UserRepository::authenticate(&db, "frank", "falsches_hash")
        .await
        .unwrap();
    assert!(falsch.is_none());

    // Unbekannter User
    let unbekannt = UserRepository::authenticate(&db, "niemand", "hash")
        .await
        .unwrap();
    assert!(unbekannt.is_none());
}

#[tokio::test]
async fn benutzer_auflisten() {
    let db = db().await;

    for name in &["user1", "user2", "user3"] {
        UserRepository::create(
            &db,
            NeuerBenutzer {
                username: name,
                password_hash: "hash",
            },
        )
        .await
        .unwrap();
    }

    let alle = UserRepository::list(&db, false).await.unwrap();
    assert!(alle.len() >= 3);
}

#[tokio::test]
async fn last_login_aktualisieren() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "grace",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    assert!(user.last_login.is_none());

    UserRepository::update_last_login(&db, user.id)
        .await
        .unwrap();

    let aktualisiert = UserRepository::get_by_id(&db, user.id)
        .await
        .unwrap()
        .unwrap();
    assert!(aktualisiert.last_login.is_some());
}
