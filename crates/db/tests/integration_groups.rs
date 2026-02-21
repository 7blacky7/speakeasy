//! Integration-Tests fuer Server- und KanalGroupRepository (In-Memory SQLite)

use speakeasy_db::{
    models::{NeueKanalGruppe, NeueServerGruppe, NeuerBenutzer, NeuerKanal},
    ChannelGroupRepository, ChannelRepository, ServerGroupRepository, SqliteDb, UserRepository,
};

async fn db() -> SqliteDb {
    SqliteDb::in_memory()
        .await
        .expect("In-Memory DB konnte nicht erstellt werden")
}

// ---------------------------------------------------------------------------
// ServerGroup-Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn server_gruppe_erstellen_und_laden() {
    let db = db().await;

    let gruppe = ServerGroupRepository::create(
        &db,
        NeueServerGruppe {
            name: "Admin",
            priority: 100,
            is_default: false,
        },
    )
    .await
    .unwrap();

    assert_eq!(gruppe.name, "Admin");
    assert_eq!(gruppe.priority, 100);

    let geladen = ServerGroupRepository::get(&db, gruppe.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(geladen.id, gruppe.id);
}

#[tokio::test]
async fn server_gruppe_mitglieder_verwalten() {
    let db = db().await;

    let gruppe = ServerGroupRepository::create(
        &db,
        NeueServerGruppe {
            name: "Moderatoren",
            priority: 50,
            is_default: false,
        },
    )
    .await
    .unwrap();

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "moderator1",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    // Mitglied hinzufuegen
    ServerGroupRepository::add_member(&db, gruppe.id, user.id)
        .await
        .unwrap();

    let gruppen_des_users = ServerGroupRepository::list_for_user(&db, user.id)
        .await
        .unwrap();
    assert_eq!(gruppen_des_users.len(), 1);
    assert_eq!(gruppen_des_users[0].name, "Moderatoren");

    // Mitglied entfernen
    let entfernt = ServerGroupRepository::remove_member(&db, gruppe.id, user.id)
        .await
        .unwrap();
    assert!(entfernt);

    let gruppen_danach = ServerGroupRepository::list_for_user(&db, user.id)
        .await
        .unwrap();
    assert!(gruppen_danach.is_empty());
}

#[tokio::test]
async fn server_gruppe_duplikat_mitglied_ignoriert() {
    let db = db().await;

    let gruppe = ServerGroupRepository::create(
        &db,
        NeueServerGruppe {
            name: "Users",
            priority: 0,
            is_default: true,
        },
    )
    .await
    .unwrap();

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "doppelt",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    // Doppeltes Hinzufuegen sollte kein Fehler sein (INSERT OR IGNORE)
    ServerGroupRepository::add_member(&db, gruppe.id, user.id)
        .await
        .unwrap();
    ServerGroupRepository::add_member(&db, gruppe.id, user.id)
        .await
        .unwrap();

    let gruppen = ServerGroupRepository::list_for_user(&db, user.id)
        .await
        .unwrap();
    assert_eq!(gruppen.len(), 1);
}

#[tokio::test]
async fn server_gruppe_standard_ermitteln() {
    let db = db().await;

    let kein_default = ServerGroupRepository::get_default(&db).await.unwrap();
    assert!(kein_default.is_none());

    ServerGroupRepository::create(
        &db,
        NeueServerGruppe {
            name: "Gaeste",
            priority: 0,
            is_default: true,
        },
    )
    .await
    .unwrap();

    let standard = ServerGroupRepository::get_default(&db).await.unwrap();
    assert!(standard.is_some());
    assert_eq!(standard.unwrap().name, "Gaeste");
}

#[tokio::test]
async fn server_gruppe_loeschen() {
    let db = db().await;

    let gruppe = ServerGroupRepository::create(
        &db,
        NeueServerGruppe {
            name: "Temporaer",
            priority: 1,
            is_default: false,
        },
    )
    .await
    .unwrap();

    let geloescht = ServerGroupRepository::delete(&db, gruppe.id).await.unwrap();
    assert!(geloescht);

    let nicht_gefunden = ServerGroupRepository::get(&db, gruppe.id).await.unwrap();
    assert!(nicht_gefunden.is_none());
}

// ---------------------------------------------------------------------------
// ChannelGroup-Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn kanal_gruppe_erstellen_und_laden() {
    let db = db().await;

    let gruppe = ChannelGroupRepository::create(&db, NeueKanalGruppe { name: "VIP" })
        .await
        .unwrap();

    assert_eq!(gruppe.name, "VIP");

    let geladen = ChannelGroupRepository::get(&db, gruppe.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(geladen.id, gruppe.id);
}

#[tokio::test]
async fn kanal_gruppe_user_zuweisung() {
    let db = db().await;

    let gruppe = ChannelGroupRepository::create(&db, NeueKanalGruppe { name: "Speaker" })
        .await
        .unwrap();

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "speaker_user",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    let kanal = ChannelRepository::create(
        &db,
        NeuerKanal {
            name: "Buehne",
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Zuweisung setzen
    ChannelGroupRepository::set_member_group(&db, user.id, kanal.id, gruppe.id)
        .await
        .unwrap();

    let zugewiesen = ChannelGroupRepository::get_for_user_in_channel(&db, user.id, kanal.id)
        .await
        .unwrap();
    assert!(zugewiesen.is_some());
    assert_eq!(zugewiesen.unwrap().name, "Speaker");

    // Zuweisung aufheben
    let entfernt = ChannelGroupRepository::remove_member_group(&db, user.id, kanal.id)
        .await
        .unwrap();
    assert!(entfernt);

    let danach = ChannelGroupRepository::get_for_user_in_channel(&db, user.id, kanal.id)
        .await
        .unwrap();
    assert!(danach.is_none());
}

#[tokio::test]
async fn kanal_gruppe_upsert() {
    let db = db().await;

    let gruppe1 = ChannelGroupRepository::create(&db, NeueKanalGruppe { name: "Gruppe1" })
        .await
        .unwrap();
    let gruppe2 = ChannelGroupRepository::create(&db, NeueKanalGruppe { name: "Gruppe2" })
        .await
        .unwrap();

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "upsert_user",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    let kanal = ChannelRepository::create(
        &db,
        NeuerKanal {
            name: "UpsertKanal",
            ..Default::default()
        },
    )
    .await
    .unwrap();

    ChannelGroupRepository::set_member_group(&db, user.id, kanal.id, gruppe1.id)
        .await
        .unwrap();

    // Upsert zur zweiten Gruppe
    ChannelGroupRepository::set_member_group(&db, user.id, kanal.id, gruppe2.id)
        .await
        .unwrap();

    let aktuell = ChannelGroupRepository::get_for_user_in_channel(&db, user.id, kanal.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(aktuell.name, "Gruppe2");
}
