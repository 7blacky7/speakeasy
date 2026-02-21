//! Integration-Tests fuer PermissionRepository und Permission-Engine (In-Memory SQLite)

use speakeasy_db::{
    models::{
        BerechtigungsWert, BerechtigungsZiel, NeueKanalGruppe, NeueServerGruppe, NeuerBenutzer,
        NeuerKanal, TriState,
    },
    ChannelGroupRepository, ChannelRepository, PermissionRepository, ServerGroupRepository,
    SqliteDb, UserRepository,
};

async fn db() -> SqliteDb {
    SqliteDb::in_memory()
        .await
        .expect("In-Memory DB konnte nicht erstellt werden")
}

fn grant() -> BerechtigungsWert {
    BerechtigungsWert::TriState(TriState::Grant)
}

fn deny() -> BerechtigungsWert {
    BerechtigungsWert::TriState(TriState::Deny)
}

#[tokio::test]
async fn berechtigung_setzen_und_laden() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "perm_user",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    let ziel = BerechtigungsZiel::Benutzer(user.id);

    PermissionRepository::set_permission(&db, &ziel, "can_speak", grant(), None)
        .await
        .unwrap();

    let perms = PermissionRepository::get_permissions(&db, &ziel, None)
        .await
        .unwrap();
    assert_eq!(perms.len(), 1);
    assert_eq!(perms[0].0, "can_speak");
    assert_eq!(perms[0].1, grant());
}

#[tokio::test]
async fn berechtigung_upsert() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "upsert_perm",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    let ziel = BerechtigungsZiel::Benutzer(user.id);

    PermissionRepository::set_permission(&db, &ziel, "can_speak", grant(), None)
        .await
        .unwrap();
    PermissionRepository::set_permission(&db, &ziel, "can_speak", deny(), None)
        .await
        .unwrap();

    let perms = PermissionRepository::get_permissions(&db, &ziel, None)
        .await
        .unwrap();
    let can_speak: Vec<_> = perms.iter().filter(|(k, _)| k == "can_speak").collect();
    assert_eq!(can_speak.len(), 1);
    assert_eq!(can_speak[0].1, deny());
}

#[tokio::test]
async fn berechtigung_entfernen() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "remove_perm",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    let ziel = BerechtigungsZiel::Benutzer(user.id);

    PermissionRepository::set_permission(&db, &ziel, "can_upload", grant(), None)
        .await
        .unwrap();

    let entfernt = PermissionRepository::remove_permission(&db, &ziel, "can_upload", None)
        .await
        .unwrap();
    assert!(entfernt);

    let perms = PermissionRepository::get_permissions(&db, &ziel, None)
        .await
        .unwrap();
    assert!(perms.is_empty());
}

#[tokio::test]
async fn berechtigung_int_limit() {
    let db = db().await;

    let ziel = BerechtigungsZiel::ServerDefault;

    PermissionRepository::set_permission(
        &db,
        &ziel,
        "max_upload_mb",
        BerechtigungsWert::IntLimit(50),
        None,
    )
    .await
    .unwrap();

    let perms = PermissionRepository::get_permissions(&db, &ziel, None)
        .await
        .unwrap();
    let upload = perms.iter().find(|(k, _)| k == "max_upload_mb").unwrap();
    assert_eq!(upload.1, BerechtigungsWert::IntLimit(50));
}

#[tokio::test]
async fn berechtigung_scope() {
    let db = db().await;

    let ziel = BerechtigungsZiel::ServerDefault;

    PermissionRepository::set_permission(
        &db,
        &ziel,
        "allowed_codecs",
        BerechtigungsWert::Scope(vec!["opus".into(), "aac".into()]),
        None,
    )
    .await
    .unwrap();

    let perms = PermissionRepository::get_permissions(&db, &ziel, None)
        .await
        .unwrap();
    let codecs = perms.iter().find(|(k, _)| k == "allowed_codecs").unwrap();
    if let BerechtigungsWert::Scope(ref s) = codecs.1 {
        assert!(s.contains(&"opus".to_string()));
        assert!(s.contains(&"aac".to_string()));
    } else {
        panic!("Erwarteter Scope-Wert");
    }
}

#[tokio::test]
async fn effektive_berechtigungen_aufloesen_individual_hat_prioritaet() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "resolv_user",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    let kanal = ChannelRepository::create(
        &db,
        NeuerKanal {
            name: "TestKanal",
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Server-Default: deny
    PermissionRepository::set_permission(
        &db,
        &BerechtigungsZiel::ServerDefault,
        "can_speak",
        deny(),
        None,
    )
    .await
    .unwrap();

    // Individual: grant (soll gewinnen)
    PermissionRepository::set_permission(
        &db,
        &BerechtigungsZiel::Benutzer(user.id),
        "can_speak",
        grant(),
        Some(kanal.id),
    )
    .await
    .unwrap();

    let effektiv = PermissionRepository::resolve_effective_permissions(&db, user.id, kanal.id)
        .await
        .unwrap();

    let can_speak = effektiv
        .iter()
        .find(|e| e.permission_key == "can_speak")
        .expect("can_speak sollte aufgeloest sein");

    assert_eq!(can_speak.wert, grant());
    assert!(
        can_speak.quelle.contains("Individual"),
        "Quelle sollte Individual sein, war: {}",
        can_speak.quelle
    );
}

#[tokio::test]
async fn effektive_berechtigungen_server_gruppe_vor_default() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "gruppe_user",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    let kanal = ChannelRepository::create(
        &db,
        NeuerKanal {
            name: "GruppenKanal",
            ..Default::default()
        },
    )
    .await
    .unwrap();

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

    ServerGroupRepository::add_member(&db, gruppe.id, user.id)
        .await
        .unwrap();

    // Server-Default: deny
    PermissionRepository::set_permission(
        &db,
        &BerechtigungsZiel::ServerDefault,
        "can_ban",
        deny(),
        None,
    )
    .await
    .unwrap();

    // Server-Gruppe: grant
    PermissionRepository::set_permission(
        &db,
        &BerechtigungsZiel::ServerGruppe(gruppe.id),
        "can_ban",
        grant(),
        None,
    )
    .await
    .unwrap();

    let effektiv = PermissionRepository::resolve_effective_permissions(&db, user.id, kanal.id)
        .await
        .unwrap();

    let can_ban = effektiv
        .iter()
        .find(|e| e.permission_key == "can_ban")
        .expect("can_ban sollte aufgeloest sein");

    assert_eq!(can_ban.wert, grant());
    assert!(
        can_ban.quelle.contains("ServerGruppe"),
        "Quelle sollte ServerGruppe sein, war: {}",
        can_ban.quelle
    );
}

#[tokio::test]
async fn effektive_berechtigungen_kanal_gruppe_vor_server_gruppe() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "kg_user",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    let kanal = ChannelRepository::create(
        &db,
        NeuerKanal {
            name: "KGKanal",
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let server_gruppe = ServerGroupRepository::create(
        &db,
        NeueServerGruppe {
            name: "SGruppe",
            priority: 100,
            is_default: false,
        },
    )
    .await
    .unwrap();

    let kanal_gruppe = ChannelGroupRepository::create(&db, NeueKanalGruppe { name: "KGruppe" })
        .await
        .unwrap();

    ServerGroupRepository::add_member(&db, server_gruppe.id, user.id)
        .await
        .unwrap();
    ChannelGroupRepository::set_member_group(&db, user.id, kanal.id, kanal_gruppe.id)
        .await
        .unwrap();

    // Server-Gruppe: grant
    PermissionRepository::set_permission(
        &db,
        &BerechtigungsZiel::ServerGruppe(server_gruppe.id),
        "can_kick",
        grant(),
        None,
    )
    .await
    .unwrap();

    // Kanal-Gruppe: deny (soll gewinnen)
    PermissionRepository::set_permission(
        &db,
        &BerechtigungsZiel::KanalGruppe(kanal_gruppe.id),
        "can_kick",
        deny(),
        Some(kanal.id),
    )
    .await
    .unwrap();

    let effektiv = PermissionRepository::resolve_effective_permissions(&db, user.id, kanal.id)
        .await
        .unwrap();

    let can_kick = effektiv
        .iter()
        .find(|e| e.permission_key == "can_kick")
        .expect("can_kick sollte aufgeloest sein");

    assert_eq!(can_kick.wert, deny());
    assert!(
        can_kick.quelle.contains("KanalGruppe"),
        "Quelle sollte KanalGruppe sein, war: {}",
        can_kick.quelle
    );
}
