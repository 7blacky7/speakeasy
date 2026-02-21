//! Integration-Tests fuer ChannelRepository (In-Memory SQLite)

use speakeasy_db::{
    models::{KanalTyp, KanalUpdate, NeuerKanal},
    ChannelRepository, SqliteDb,
};

async fn db() -> SqliteDb {
    SqliteDb::in_memory().await.expect("In-Memory DB konnte nicht erstellt werden")
}

#[tokio::test]
async fn kanal_erstellen_und_laden() {
    let db = db().await;

    let kanal = ChannelRepository::create(&db, NeuerKanal {
        name: "Lobby",
        channel_type: KanalTyp::Voice,
        is_default: true,
        sort_order: 0,
        ..Default::default()
    })
    .await
    .unwrap();

    assert_eq!(kanal.name, "Lobby");
    assert!(kanal.is_default);
    assert_eq!(kanal.channel_type, KanalTyp::Voice);

    let geladen = ChannelRepository::get_by_id(&db, kanal.id).await.unwrap().unwrap();
    assert_eq!(geladen.id, kanal.id);
    assert_eq!(geladen.name, "Lobby");
}

#[tokio::test]
async fn kanal_hierarchie() {
    let db = db().await;

    let eltern = ChannelRepository::create(&db, NeuerKanal {
        name: "Eltern-Kanal",
        sort_order: 0,
        ..Default::default()
    })
    .await
    .unwrap();

    let kind1 = ChannelRepository::create(&db, NeuerKanal {
        name: "Kind-1",
        parent_id: Some(eltern.id),
        sort_order: 1,
        ..Default::default()
    })
    .await
    .unwrap();

    let kind2 = ChannelRepository::create(&db, NeuerKanal {
        name: "Kind-2",
        parent_id: Some(eltern.id),
        sort_order: 2,
        ..Default::default()
    })
    .await
    .unwrap();

    let kinder = ChannelRepository::get_children(&db, eltern.id).await.unwrap();
    assert_eq!(kinder.len(), 2);
    let names: Vec<&str> = kinder.iter().map(|k| k.name.as_str()).collect();
    assert!(names.contains(&"Kind-1"));
    assert!(names.contains(&"Kind-2"));

    assert_eq!(kind1.parent_id, Some(eltern.id));
    assert_eq!(kind2.parent_id, Some(eltern.id));
}

#[tokio::test]
async fn kanal_auflisten_sortiert() {
    let db = db().await;

    ChannelRepository::create(&db, NeuerKanal { name: "C", sort_order: 3, ..Default::default() }).await.unwrap();
    ChannelRepository::create(&db, NeuerKanal { name: "A", sort_order: 1, ..Default::default() }).await.unwrap();
    ChannelRepository::create(&db, NeuerKanal { name: "B", sort_order: 2, ..Default::default() }).await.unwrap();

    let kanaele = ChannelRepository::list(&db).await.unwrap();
    assert!(kanaele.len() >= 3);

    // Sortierung nach sort_order
    let sort_orders: Vec<i64> = kanaele.iter().map(|k| k.sort_order).collect();
    let mut erwartet = sort_orders.clone();
    erwartet.sort();
    assert_eq!(sort_orders, erwartet);
}

#[tokio::test]
async fn kanal_aktualisieren() {
    let db = db().await;

    let kanal = ChannelRepository::create(&db, NeuerKanal {
        name: "Alt",
        ..Default::default()
    })
    .await
    .unwrap();

    let aktualisiert = ChannelRepository::update(
        &db,
        kanal.id,
        KanalUpdate {
            name: Some("Neu".into()),
            topic: Some(Some("Neues Thema".into())),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(aktualisiert.name, "Neu");
    assert_eq!(aktualisiert.topic.as_deref(), Some("Neues Thema"));
}

#[tokio::test]
async fn kanal_loeschen() {
    let db = db().await;

    let kanal = ChannelRepository::create(&db, NeuerKanal { name: "Loeschen", ..Default::default() })
        .await
        .unwrap();

    let geloescht = ChannelRepository::delete(&db, kanal.id).await.unwrap();
    assert!(geloescht);

    let nicht_gefunden = ChannelRepository::get_by_id(&db, kanal.id).await.unwrap();
    assert!(nicht_gefunden.is_none());
}

#[tokio::test]
async fn standard_kanal_ermitteln() {
    let db = db().await;

    let kein_default = ChannelRepository::get_default(&db).await.unwrap();
    assert!(kein_default.is_none());

    ChannelRepository::create(&db, NeuerKanal {
        name: "Standard",
        is_default: true,
        ..Default::default()
    })
    .await
    .unwrap();

    let standard = ChannelRepository::get_default(&db).await.unwrap();
    assert!(standard.is_some());
    assert_eq!(standard.unwrap().name, "Standard");
}

#[tokio::test]
async fn text_kanal_typ() {
    let db = db().await;

    let kanal = ChannelRepository::create(&db, NeuerKanal {
        name: "Chat",
        channel_type: KanalTyp::Text,
        ..Default::default()
    })
    .await
    .unwrap();

    assert_eq!(kanal.channel_type, KanalTyp::Text);

    let geladen = ChannelRepository::get_by_id(&db, kanal.id).await.unwrap().unwrap();
    assert_eq!(geladen.channel_type, KanalTyp::Text);
}
