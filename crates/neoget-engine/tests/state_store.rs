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

    let identity = PackageIdentity::new(ProviderId::new("apt").unwrap(), "curl", None).unwrap();
    store.pin(&identity, "8.0").unwrap();
    store.pin(&identity, "8.0").unwrap();
    assert!(store.pin(&identity, "9.0").is_err());
    let first = store
        .save_bundle_plan(std::slice::from_ref(&identity))
        .unwrap();
    let repeated = store
        .save_bundle_plan(std::slice::from_ref(&identity))
        .unwrap();
    assert_eq!(first, repeated);
    drop(store);
    let reopened = StateStore::open(&database).unwrap();
    assert_eq!(reopened.save_bundle_plan(&[identity]).unwrap(), first);
}
