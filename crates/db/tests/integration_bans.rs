//! Integration-Tests fuer BanRepository (In-Memory SQLite)

use chrono::{Duration, Utc};
use speakeasy_db::{
    models::{NeuerBan, NeuerBenutzer},
    BanRepository, SqliteDb, UserRepository,
};

async fn db() -> SqliteDb {
    SqliteDb::in_memory()
        .await
        .expect("In-Memory DB konnte nicht erstellt werden")
}

#[tokio::test]
async fn ban_erstellen_und_laden() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "troll",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    let ban = BanRepository::create(
        &db,
        NeuerBan {
            user_id: Some(user.id),
            ip: None,
            reason: "Regelverstos",
            banned_by: None,
            expires_at: None,
        },
    )
    .await
    .unwrap();

    assert_eq!(ban.user_id, Some(user.id));
    assert_eq!(ban.reason, "Regelverstos");
    assert!(ban.expires_at.is_none());

    let geladen = BanRepository::get(&db, ban.id).await.unwrap().unwrap();
    assert_eq!(geladen.id, ban.id);
}

#[tokio::test]
async fn ban_pruefen_user_id() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "gesperrt",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    BanRepository::create(
        &db,
        NeuerBan {
            user_id: Some(user.id),
            ip: None,
            reason: "Spam",
            banned_by: None,
            expires_at: None,
        },
    )
    .await
    .unwrap();

    let ist_gebannt = BanRepository::is_banned(&db, Some(user.id), None)
        .await
        .unwrap();
    assert!(ist_gebannt.is_some());

    let anderer_user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "unschuldig",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();
    let nicht_gebannt = BanRepository::is_banned(&db, Some(anderer_user.id), None)
        .await
        .unwrap();
    assert!(nicht_gebannt.is_none());
}

#[tokio::test]
async fn ban_pruefen_ip() {
    let db = db().await;

    BanRepository::create(
        &db,
        NeuerBan {
            user_id: None,
            ip: Some("192.168.1.100"),
            reason: "IP-Ban",
            banned_by: None,
            expires_at: None,
        },
    )
    .await
    .unwrap();

    let ist_gebannt = BanRepository::is_banned(&db, None, Some("192.168.1.100"))
        .await
        .unwrap();
    assert!(ist_gebannt.is_some());

    let andere_ip = BanRepository::is_banned(&db, None, Some("10.0.0.1"))
        .await
        .unwrap();
    assert!(andere_ip.is_none());
}

#[tokio::test]
async fn ban_entfernen() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "begnadigt",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    let ban = BanRepository::create(
        &db,
        NeuerBan {
            user_id: Some(user.id),
            ip: None,
            reason: "Versehen",
            banned_by: None,
            expires_at: None,
        },
    )
    .await
    .unwrap();

    let entfernt = BanRepository::remove(&db, ban.id).await.unwrap();
    assert!(entfernt);

    let nach_entfernung = BanRepository::is_banned(&db, Some(user.id), None)
        .await
        .unwrap();
    assert!(nach_entfernung.is_none());
}

#[tokio::test]
async fn ban_abgelaufener_wird_ignoriert() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "abgelaufen_user",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    BanRepository::create(
        &db,
        NeuerBan {
            user_id: Some(user.id),
            ip: None,
            reason: "Temporaer",
            banned_by: None,
            expires_at: Some(Utc::now() - Duration::hours(1)),
        },
    )
    .await
    .unwrap();

    let ist_gebannt = BanRepository::is_banned(&db, Some(user.id), None)
        .await
        .unwrap();
    assert!(
        ist_gebannt.is_none(),
        "Abgelaufener Ban sollte nicht als aktiv gelten"
    );
}

#[tokio::test]
async fn ban_cleanup_abgelaufene() {
    let db = db().await;

    BanRepository::create(
        &db,
        NeuerBan {
            user_id: None,
            ip: Some("1.2.3.4"),
            reason: "Alt",
            banned_by: None,
            expires_at: Some(Utc::now() - Duration::days(1)),
        },
    )
    .await
    .unwrap();

    BanRepository::create(
        &db,
        NeuerBan {
            user_id: None,
            ip: Some("5.6.7.8"),
            reason: "Aktuell",
            banned_by: None,
            expires_at: None,
        },
    )
    .await
    .unwrap();

    let geloescht = BanRepository::cleanup_expired(&db).await.unwrap();
    assert_eq!(geloescht, 1);

    let alle = BanRepository::list(&db, false).await.unwrap();
    assert_eq!(alle.len(), 1);
    assert_eq!(alle[0].ip.as_deref(), Some("5.6.7.8"));
}

#[tokio::test]
async fn ban_auflisten_nur_aktive() {
    let db = db().await;

    BanRepository::create(
        &db,
        NeuerBan {
            user_id: None,
            ip: Some("9.9.9.9"),
            reason: "Abgelaufen",
            banned_by: None,
            expires_at: Some(Utc::now() - Duration::seconds(1)),
        },
    )
    .await
    .unwrap();

    BanRepository::create(
        &db,
        NeuerBan {
            user_id: None,
            ip: Some("8.8.8.8"),
            reason: "Aktiv",
            banned_by: None,
            expires_at: None,
        },
    )
    .await
    .unwrap();

    let nur_aktive = BanRepository::list(&db, true).await.unwrap();
    assert_eq!(nur_aktive.len(), 1);
    assert_eq!(nur_aktive[0].ip.as_deref(), Some("8.8.8.8"));

    let alle = BanRepository::list(&db, false).await.unwrap();
    assert_eq!(alle.len(), 2);
}
