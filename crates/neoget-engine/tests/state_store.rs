use neoget_core::{PackageIdentity, ProviderId};
use neoget_engine::{Source, StateStore};

#[test]
fn migrations_sources_atomic_import_pins_and_plans_are_persistent_and_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("state.sqlite");
    let source = Source {
        driver: "apt".into(),
        scope: "user".into(),
        name: "main".into(),
        location: "https://example.invalid/repo".into(),
    };
    let mut store = StateStore::open(&database).unwrap();
    store.import_sources(std::slice::from_ref(&source)).unwrap();
    store.import_sources(std::slice::from_ref(&source)).unwrap();
    assert_eq!(store.sources().unwrap(), vec![source]);
    let invalid = Source {
        driver: "unknown".into(),
        scope: "user".into(),
        name: "bad".into(),
        location: "x".into(),
    };
    assert!(store.import_sources(&[invalid]).is_err());
    assert_eq!(store.sources().unwrap().len(), 1);
    store.remove_source("apt", "user", "main").unwrap();
    store.remove_source("apt", "user", "main").unwrap();
    assert!(store.sources().unwrap().is_empty());
    assert_eq!(
        store
            .update_source("apt", "user", "missing", "new-location")
            .unwrap_err()
            .to_string(),
        "SOURCE_NOT_FOUND: source key does not exist"
    );

    let identity = PackageIdentity::new(ProviderId::new("apt").unwrap(), "curl", None).unwrap();
    store.pin(&identity, "8.0").unwrap();
    store.pin(&identity, "8.0").unwrap();
    assert!(store.pin(&identity, "9.0").is_err());
    assert_eq!(
        store.pin_for("apt", "curl").unwrap().unwrap().version,
        "8.0"
    );
    assert_eq!(store.pins().unwrap().len(), 1);
    let first = store
        .save_bundle_plan(std::slice::from_ref(&identity))
        .unwrap();
    let repeated = store
        .save_bundle_plan(std::slice::from_ref(&identity))
        .unwrap();
    assert_eq!(first, repeated);
    drop(store);
    let reopened = StateStore::open(&database).unwrap();
    assert_eq!(
        reopened.load_bundle_plan(&first.digest).unwrap(),
        Some(first)
    );
    assert_eq!(
        reopened.pin_for("apt", "curl").unwrap().unwrap().version,
        "8.0"
    );
    assert_eq!(reopened.pins().unwrap().len(), 1);
}

#[test]
fn schema_versions_are_rejected_and_concurrent_connections_serialize_writes() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("state.sqlite");
    let first = StateStore::open(&database).unwrap();
    let second = StateStore::open(&database).unwrap();
    let source = Source {
        driver: "apt".into(),
        scope: "user".into(),
        name: "main".into(),
        location: "repo".into(),
    };
    let mut writer = first;
    writer
        .import_sources(std::slice::from_ref(&source))
        .unwrap();
    assert_eq!(second.sources().unwrap(), vec![source]);
    drop(writer);
    drop(second);

    let future = directory.path().join("future.sqlite");
    let connection = rusqlite::Connection::open(&future).unwrap();
    connection.pragma_update(None, "user_version", 99).unwrap();
    drop(connection);
    match StateStore::open(&future) {
        Err(error) => assert_eq!(
            error.to_string(),
            "STATE_SCHEMA_FUTURE: database schema is newer than this application"
        ),
        Ok(_) => panic!("future schema was accepted"),
    }
}

#[test]
fn source_import_database_failure_rolls_back_prior_rows() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("rollback.sqlite");
    {
        let mut store = StateStore::open(&database).unwrap();
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection.execute_batch("CREATE TRIGGER reject_bad BEFORE INSERT ON sources WHEN NEW.name='bad' BEGIN SELECT RAISE(ABORT,'rejected'); END;").unwrap();
        let sources = [
            Source {
                driver: "apt".into(),
                scope: "user".into(),
                name: "good".into(),
                location: "repo".into(),
            },
            Source {
                driver: "apt".into(),
                scope: "user".into(),
                name: "bad".into(),
                location: "repo".into(),
            },
        ];
        assert!(store.import_sources(&sources).is_err());
        assert!(store.sources().unwrap().is_empty());
    }
}
