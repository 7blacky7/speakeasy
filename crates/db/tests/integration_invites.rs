//! Integration-Tests fuer InviteRepository (In-Memory SQLite)

use chrono::{Duration, Utc};
use speakeasy_db::{
    models::{NeueEinladung, NeuerBenutzer},
    DbError, InviteRepository, SqliteDb, UserRepository,
};

async fn db() -> SqliteDb {
    SqliteDb::in_memory().await.expect("In-Memory DB konnte nicht erstellt werden")
}

async fn erstelle_user(db: &SqliteDb, name: &str) -> uuid::Uuid {
    UserRepository::create(db, NeuerBenutzer { username: name, password_hash: "hash" })
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn einladung_erstellen_und_laden() {
    let db = db().await;
    let user_id = erstelle_user(&db, "einlader").await;

    let einladung = InviteRepository::create(&db, NeueEinladung {
        code: "ABC123",
        channel_id: None,
        assigned_group_id: None,
        max_uses: 10,
        expires_at: None,
        created_by: user_id,
    })
    .await
    .unwrap();

    assert_eq!(einladung.code, "ABC123");
    assert_eq!(einladung.max_uses, 10);
    assert_eq!(einladung.used_count, 0);

    let geladen = InviteRepository::get(&db, einladung.id).await.unwrap().unwrap();
    assert_eq!(geladen.id, einladung.id);

    let per_code = InviteRepository::get_by_code(&db, "ABC123").await.unwrap().unwrap();
    assert_eq!(per_code.id, einladung.id);
}

#[tokio::test]
async fn einladung_code_unique() {
    let db = db().await;
    let user_id = erstelle_user(&db, "einlader2").await;

    InviteRepository::create(&db, NeueEinladung {
        code: "UNIQUE1",
        channel_id: None,
        assigned_group_id: None,
        max_uses: 0,
        expires_at: None,
        created_by: user_id,
    })
    .await
    .unwrap();

    let err = InviteRepository::create(&db, NeueEinladung {
        code: "UNIQUE1",
        channel_id: None,
        assigned_group_id: None,
        max_uses: 0,
        expires_at: None,
        created_by: user_id,
    })
    .await;

    assert!(err.is_err());
    assert!(err.unwrap_err().ist_eindeutigkeit());
}

#[tokio::test]
async fn einladung_verwenden_zaehler() {
    let db = db().await;
    let user_id = erstelle_user(&db, "einlader3").await;

    InviteRepository::create(&db, NeueEinladung {
        code: "USE123",
        channel_id: None,
        assigned_group_id: None,
        max_uses: 5,
        expires_at: None,
        created_by: user_id,
    })
    .await
    .unwrap();

    for i in 1..=3 {
        let result = InviteRepository::use_invite(&db, "USE123").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().used_count, i);
    }
}

#[tokio::test]
async fn einladung_erschoepft() {
    let db = db().await;
    let user_id = erstelle_user(&db, "einlader4").await;

    InviteRepository::create(&db, NeueEinladung {
        code: "LIMIT1",
        channel_id: None,
        assigned_group_id: None,
        max_uses: 2,
        expires_at: None,
        created_by: user_id,
    })
    .await
    .unwrap();

    InviteRepository::use_invite(&db, "LIMIT1").await.unwrap();
    InviteRepository::use_invite(&db, "LIMIT1").await.unwrap();

    let err = InviteRepository::use_invite(&db, "LIMIT1").await;
    assert!(err.is_err());
    assert!(matches!(err.unwrap_err(), DbError::EinladungErschoepft));
}

#[tokio::test]
async fn einladung_abgelaufen() {
    let db = db().await;
    let user_id = erstelle_user(&db, "einlader5").await;

    InviteRepository::create(&db, NeueEinladung {
        code: "EXPIRE1",
        channel_id: None,
        assigned_group_id: None,
        max_uses: 100,
        expires_at: Some(Utc::now() - Duration::hours(1)),
        created_by: user_id,
    })
    .await
    .unwrap();

    let err = InviteRepository::use_invite(&db, "EXPIRE1").await;
    assert!(err.is_err());
    assert!(matches!(err.unwrap_err(), DbError::EinladungUngueltig));
}

#[tokio::test]
async fn einladung_unbegrenzt() {
    let db = db().await;
    let user_id = erstelle_user(&db, "einlader6").await;

    InviteRepository::create(&db, NeueEinladung {
        code: "UNLIMITED",
        channel_id: None,
        assigned_group_id: None,
        max_uses: 0,
        expires_at: None,
        created_by: user_id,
    })
    .await
    .unwrap();

    for _ in 0..20 {
        let result = InviteRepository::use_invite(&db, "UNLIMITED").await.unwrap();
        assert!(result.is_some());
    }
}

#[tokio::test]
async fn einladung_widerrufen() {
    let db = db().await;
    let user_id = erstelle_user(&db, "einlader7").await;

    let einladung = InviteRepository::create(&db, NeueEinladung {
        code: "REVOKE1",
        channel_id: None,
        assigned_group_id: None,
        max_uses: 0,
        expires_at: None,
        created_by: user_id,
    })
    .await
    .unwrap();

    let widerrufen = InviteRepository::revoke(&db, einladung.id).await.unwrap();
    assert!(widerrufen);

    let nicht_gefunden = InviteRepository::get(&db, einladung.id).await.unwrap();
    assert!(nicht_gefunden.is_none());
}

#[tokio::test]
async fn einladungen_auflisten() {
    let db = db().await;
    let user_id = erstelle_user(&db, "einlader8").await;
    let anderer_id = erstelle_user(&db, "anderer_einlader").await;

    for i in 0..3 {
        InviteRepository::create(&db, NeueEinladung {
            code: &format!("MY{i}"),
            channel_id: None,
            assigned_group_id: None,
            max_uses: 0,
            expires_at: None,
            created_by: user_id,
        })
        .await
        .unwrap();
    }

    InviteRepository::create(&db, NeueEinladung {
        code: "OTHER",
        channel_id: None,
        assigned_group_id: None,
        max_uses: 0,
        expires_at: None,
        created_by: anderer_id,
    })
    .await
    .unwrap();

    let meine = InviteRepository::list(&db, Some(user_id)).await.unwrap();
    assert_eq!(meine.len(), 3);
    assert!(meine.iter().all(|e| e.created_by == user_id));

    let alle = InviteRepository::list(&db, None).await.unwrap();
    assert!(alle.len() >= 4);
}
