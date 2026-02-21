//! Integration-Tests fuer AuditLogRepository (In-Memory SQLite)

use speakeasy_db::{
    models::{AuditLogFilter, NeuerBenutzer},
    AuditLogRepository, SqliteDb, UserRepository,
};

async fn db() -> SqliteDb {
    SqliteDb::in_memory()
        .await
        .expect("In-Memory DB konnte nicht erstellt werden")
}

#[tokio::test]
async fn audit_ereignis_protokollieren() {
    let db = db().await;

    let eintrag = AuditLogRepository::log_event(
        &db,
        None,
        "server.start",
        None,
        None,
        serde_json::json!({"version": "0.1.0"}),
    )
    .await
    .unwrap();

    assert_eq!(eintrag.action, "server.start");
    assert!(eintrag.actor_id.is_none());
    assert_eq!(eintrag.details["version"], "0.1.0");
}

#[tokio::test]
async fn audit_mit_actor() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "admin",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    let eintrag = AuditLogRepository::log_event(
        &db,
        Some(user.id),
        "user.ban",
        Some("user"),
        Some("target-user-id"),
        serde_json::json!({"reason": "Spam"}),
    )
    .await
    .unwrap();

    assert_eq!(eintrag.actor_id, Some(user.id));
    assert_eq!(eintrag.action, "user.ban");
    assert_eq!(eintrag.target_type.as_deref(), Some("user"));
}

#[tokio::test]
async fn audit_ereignisse_auflisten() {
    let db = db().await;

    for i in 0..5 {
        AuditLogRepository::log_event(
            &db,
            None,
            "test.action",
            None,
            None,
            serde_json::json!({"index": i}),
        )
        .await
        .unwrap();
    }

    let alle = AuditLogRepository::list_events(&db, AuditLogFilter::default())
        .await
        .unwrap();
    assert!(alle.len() >= 5);
}

#[tokio::test]
async fn audit_filter_nach_action() {
    let db = db().await;

    AuditLogRepository::log_event(
        &db,
        None,
        "user.create",
        None,
        None,
        serde_json::Value::Null,
    )
    .await
    .unwrap();
    AuditLogRepository::log_event(
        &db,
        None,
        "user.delete",
        None,
        None,
        serde_json::Value::Null,
    )
    .await
    .unwrap();
    AuditLogRepository::log_event(
        &db,
        None,
        "user.create",
        None,
        None,
        serde_json::Value::Null,
    )
    .await
    .unwrap();

    let filter = AuditLogFilter {
        action: Some("user.create".into()),
        ..Default::default()
    };

    let ergebnisse = AuditLogRepository::list_events(&db, filter).await.unwrap();
    assert_eq!(ergebnisse.len(), 2);
    assert!(ergebnisse.iter().all(|e| e.action == "user.create"));
}

#[tokio::test]
async fn audit_filter_nach_actor() {
    let db = db().await;

    let user = UserRepository::create(
        &db,
        NeuerBenutzer {
            username: "logger",
            password_hash: "hash",
        },
    )
    .await
    .unwrap();

    AuditLogRepository::log_event(
        &db,
        Some(user.id),
        "action.a",
        None,
        None,
        serde_json::Value::Null,
    )
    .await
    .unwrap();
    AuditLogRepository::log_event(&db, None, "action.b", None, None, serde_json::Value::Null)
        .await
        .unwrap();
    AuditLogRepository::log_event(
        &db,
        Some(user.id),
        "action.c",
        None,
        None,
        serde_json::Value::Null,
    )
    .await
    .unwrap();

    let filter = AuditLogFilter {
        actor_id: Some(user.id),
        ..Default::default()
    };

    let ergebnisse = AuditLogRepository::list_events(&db, filter).await.unwrap();
    assert_eq!(ergebnisse.len(), 2);
    assert!(ergebnisse.iter().all(|e| e.actor_id == Some(user.id)));
}

#[tokio::test]
async fn audit_limit_und_offset() {
    let db = db().await;

    for i in 0..10 {
        AuditLogRepository::log_event(
            &db,
            None,
            "paginate.test",
            None,
            None,
            serde_json::json!({"i": i}),
        )
        .await
        .unwrap();
    }

    let seite1 = AuditLogRepository::list_events(
        &db,
        AuditLogFilter {
            action: Some("paginate.test".into()),
            limit: Some(3),
            offset: Some(0),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(seite1.len(), 3);

    let seite2 = AuditLogRepository::list_events(
        &db,
        AuditLogFilter {
            action: Some("paginate.test".into()),
            limit: Some(3),
            offset: Some(3),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(seite2.len(), 3);

    let ids1: Vec<_> = seite1.iter().map(|e| e.id).collect();
    let ids2: Vec<_> = seite2.iter().map(|e| e.id).collect();
    assert!(!ids1.iter().any(|id| ids2.contains(id)));
}

#[tokio::test]
async fn audit_ereignisse_zaehlen() {
    let db = db().await;

    for _ in 0..7 {
        AuditLogRepository::log_event(&db, None, "count.test", None, None, serde_json::Value::Null)
            .await
            .unwrap();
    }

    let anzahl = AuditLogRepository::count_events(
        &db,
        AuditLogFilter {
            action: Some("count.test".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(anzahl, 7);
}
