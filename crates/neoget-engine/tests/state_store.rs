use neoget_core::{PackageIdentity, ProviderId};
use neoget_engine::{Source, StateStore};

fn schema_snapshot(connection: &rusqlite::Connection) -> (i64, Vec<(String, Option<String>)>) {
    let version = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    let mut statement = connection
        .prepare("SELECT name,sql FROM sqlite_schema ORDER BY name")
        .unwrap();
    let objects = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    (version, objects)
}

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
fn schema_versions_are_rejected_and_connections_observe_committed_writes() {
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

#[test]
fn original_v1_migration_is_still_accepted() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("original.sqlite");
    let connection = rusqlite::Connection::open(&database).unwrap();
    // Independent fixture: the exact batch used before canonical DDL validation.
    connection.execute_batch(
        "CREATE TABLE schema_migrations(version INTEGER NOT NULL PRIMARY KEY CHECK(version=1));
         CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL,PRIMARY KEY(driver,scope,name));
         CREATE TABLE pins(driver TEXT NOT NULL,name TEXT NOT NULL,version TEXT NOT NULL,PRIMARY KEY(driver,name));
         CREATE TABLE bundle_plans(digest TEXT NOT NULL PRIMARY KEY,intents BLOB NOT NULL);
         INSERT INTO schema_migrations(version) VALUES(1);
         INSERT INTO sources VALUES('apt','user','main','repo');
         PRAGMA user_version=1;",
    ).unwrap();
    let before = schema_snapshot(&connection);
    let store = StateStore::open(&database).unwrap();
    assert_eq!(schema_snapshot(&connection), before);
    assert_eq!(store.sources().unwrap()[0].location, "repo");
    let package = PackageIdentity::new(ProviderId::new("apt").unwrap(), "curl", None).unwrap();
    store.pin(&package, "8.0").unwrap();
    assert_eq!(store.pins().unwrap().len(), 1);
}

#[test]
fn unsupported_table_constraints_are_rejected_on_open() {
    for (table, definition) in [
        (
            "sources",
            "CREATE TABLE sources(driver TEXT NOT NULL CHECK(driver='blocked'),scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL,PRIMARY KEY(driver,scope,name))",
        ),
        (
            "pins",
            "CREATE TABLE pins(driver TEXT NOT NULL,name TEXT NOT NULL,version TEXT NOT NULL CHECK(version='blocked'),PRIMARY KEY(driver,name))",
        ),
        (
            "bundle_plans",
            "CREATE TABLE bundle_plans(digest TEXT NOT NULL PRIMARY KEY,intents BLOB NOT NULL CHECK(length(intents)=0))",
        ),
        (
            "schema_migrations",
            "CREATE TABLE schema_migrations(version INTEGER NOT NULL PRIMARY KEY CHECK(version=1) CHECK(version<2))",
        ),
        (
            "schema_migrations",
            "CREATE TABLE schema_migrations(version INTEGER NOT NULL PRIMARY KEY)",
        ),
        (
            "sources",
            "CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL,PRIMARY KEY(driver,scope,name),FOREIGN KEY(driver,name) REFERENCES pins(driver,name))",
        ),
        (
            "pins",
            "CREATE TABLE pins(driver TEXT NOT NULL,name TEXT NOT NULL,version TEXT NOT NULL,PRIMARY KEY(driver,name),FOREIGN KEY(driver) REFERENCES schema_migrations(version))",
        ),
        (
            "bundle_plans",
            "CREATE TABLE bundle_plans(digest TEXT NOT NULL PRIMARY KEY REFERENCES schema_migrations(version),intents BLOB NOT NULL)",
        ),
        (
            "schema_migrations",
            "CREATE TABLE schema_migrations(version INTEGER NOT NULL PRIMARY KEY CHECK(version=1) REFERENCES pins(name))",
        ),
        (
            "sources",
            "CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL COLLATE NOCASE,PRIMARY KEY(driver,scope,name))",
        ),
        (
            "pins",
            "CREATE TABLE pins(driver TEXT NOT NULL,name TEXT NOT NULL,version TEXT NOT NULL COLLATE NOCASE,PRIMARY KEY(driver,name))",
        ),
        (
            "bundle_plans",
            "CREATE TABLE bundle_plans(digest TEXT NOT NULL PRIMARY KEY,intents BLOB NOT NULL COLLATE NOCASE)",
        ),
        (
            "schema_migrations",
            "CREATE TABLE schema_migrations(version INTEGER NOT NULL PRIMARY KEY COLLATE NOCASE CHECK(version=1))",
        ),
        (
            "sources",
            "CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL DEFAULT 'repo',PRIMARY KEY(driver,scope,name))",
        ),
        (
            "pins",
            "CREATE TABLE pins(driver TEXT NOT NULL,name TEXT NOT NULL,version TEXT NOT NULL DEFAULT '1',PRIMARY KEY(driver,name))",
        ),
        (
            "bundle_plans",
            "CREATE TABLE bundle_plans(digest TEXT NOT NULL PRIMARY KEY,intents BLOB NOT NULL DEFAULT x'00')",
        ),
        (
            "schema_migrations",
            "CREATE TABLE schema_migrations(version INTEGER NOT NULL PRIMARY KEY DEFAULT 1 CHECK(version=1))",
        ),
        (
            "sources",
            "CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL,PRIMARY KEY(driver,scope,name) ON CONFLICT REPLACE)",
        ),
        (
            "pins",
            "CREATE TABLE pins(driver TEXT NOT NULL,name TEXT NOT NULL,version TEXT NOT NULL,PRIMARY KEY(driver,name) ON CONFLICT IGNORE)",
        ),
        (
            "bundle_plans",
            "CREATE TABLE bundle_plans(digest TEXT NOT NULL PRIMARY KEY ON CONFLICT REPLACE,intents BLOB NOT NULL)",
        ),
        (
            "schema_migrations",
            "CREATE TABLE schema_migrations(version INTEGER NOT NULL PRIMARY KEY ON CONFLICT IGNORE CHECK(version=1))",
        ),
        (
            "sources",
            "CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL GENERATED ALWAYS AS ('repo') STORED,PRIMARY KEY(driver,scope,name))",
        ),
        (
            "pins",
            "CREATE TABLE pins(driver TEXT NOT NULL,name TEXT NOT NULL,version TEXT NOT NULL GENERATED ALWAYS AS ('1') VIRTUAL,PRIMARY KEY(driver,name))",
        ),
        (
            "bundle_plans",
            "CREATE TABLE bundle_plans(digest TEXT NOT NULL PRIMARY KEY,intents BLOB NOT NULL GENERATED ALWAYS AS (x'00') STORED)",
        ),
        (
            "pins",
            "CREATE TABLE pins(driver TEXT NOT NULL,name TEXT NOT NULL,version TEXT NOT NULL,PRIMARY KEY(driver,name)) WITHOUT ROWID",
        ),
        (
            "bundle_plans",
            "CREATE TABLE bundle_plans(digest TEXT NOT NULL PRIMARY KEY,intents BLOB NOT NULL) STRICT",
        ),
        // v1 is deliberately fail-closed, even for equivalent hand-authored SQL.
        (
            "pins",
            "CREATE TABLE pins (driver TEXT NOT NULL,name TEXT NOT NULL,version TEXT NOT NULL,PRIMARY KEY(driver,name))",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("constraints.sqlite");
        drop(StateStore::open(&database).unwrap());
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .pragma_update(None, "foreign_keys", false)
            .unwrap();
        connection
            .execute_batch(&format!("DROP TABLE {table}; {definition}"))
            .unwrap();
        if table == "schema_migrations" {
            // Keep history valid so a rejection cannot pass vacuously on missing rows.
            connection
                .execute("INSERT INTO schema_migrations VALUES(1)", [])
                .unwrap();
        }
        let before = schema_snapshot(&connection);
        match StateStore::open(&database) {
            Err(error) => assert!(
                error.to_string().starts_with("STATE_SCHEMA_INVALID:"),
                "{definition}: {error}"
            ),
            Ok(_) => panic!("unsupported table constraint accepted: {definition}"),
        }
        assert_eq!(schema_snapshot(&connection), before);
    }
}

#[test]
fn external_indexes_and_triggers_on_every_store_table_are_rejected() {
    for (table, column) in [
        ("schema_migrations", "version"),
        ("sources", "location"),
        ("pins", "version"),
        ("bundle_plans", "intents"),
    ] {
        for definition in [
            format!("CREATE INDEX hidden_restriction ON {table}({column})"),
            format!("CREATE UNIQUE INDEX hidden_restriction ON {table}({column})"),
            format!(
                "CREATE TRIGGER hidden_restriction BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT,'blocked'); END"
            ),
            format!(
                "CREATE TRIGGER hidden_restriction BEFORE INSERT ON {} BEGIN SELECT RAISE(ABORT,'blocked'); END",
                table.to_ascii_uppercase()
            ),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let database = directory.path().join("external.sqlite");
            drop(StateStore::open(&database).unwrap());
            let connection = rusqlite::Connection::open(&database).unwrap();
            connection.execute_batch(&definition).unwrap();
            let before = schema_snapshot(&connection);
            match StateStore::open(&database) {
                Err(error) => assert!(
                    error.to_string().starts_with("STATE_SCHEMA_INVALID:"),
                    "{definition}: {error}"
                ),
                Ok(_) => panic!("external schema restriction accepted: {definition}"),
            }
            assert_eq!(schema_snapshot(&connection), before);
        }
    }
}

#[test]
fn malformed_named_schemas_are_rejected_before_use() {
    for definition in [
        "CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL)",
        "CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location INTEGER NOT NULL,PRIMARY KEY(driver,scope,name))",
        "CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT,PRIMARY KEY(driver,scope,name))",
        "CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL,PRIMARY KEY(name,scope,driver))",
        "CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL,UNIQUE(driver,scope,name))",
        "CREATE TABLE sources(driver TEXT NOT NULL COLLATE NOCASE,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL,PRIMARY KEY(driver,scope,name))",
        "CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL,extra TEXT GENERATED ALWAYS AS (name) VIRTUAL,PRIMARY KEY(driver,scope,name))",
        "CREATE TABLE pins(driver TEXT NOT NULL,name TEXT NOT NULL,version TEXT NOT NULL)",
        "CREATE TABLE pins(driver TEXT NOT NULL,name TEXT NOT NULL,version BLOB NOT NULL,PRIMARY KEY(driver,name))",
        "CREATE TABLE pins(driver TEXT NOT NULL,name TEXT,version TEXT NOT NULL,PRIMARY KEY(driver,name))",
        "CREATE TABLE pins(driver TEXT NOT NULL,name TEXT NOT NULL UNIQUE,version TEXT NOT NULL,PRIMARY KEY(driver,name))",
        "CREATE TABLE bundle_plans(digest TEXT NOT NULL,intents BLOB NOT NULL)",
        "CREATE TABLE bundle_plans(digest TEXT NOT NULL PRIMARY KEY,intents TEXT NOT NULL)",
        "CREATE TABLE bundle_plans(digest TEXT PRIMARY KEY,intents BLOB NOT NULL)",
        "CREATE TABLE schema_migrations(version INTEGER NOT NULL)",
        "CREATE TABLE schema_migrations(version TEXT NOT NULL PRIMARY KEY)",
        "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY)",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("malformed.sqlite");
        drop(StateStore::open(&database).unwrap());
        let connection = rusqlite::Connection::open(&database).unwrap();
        let table = definition
            .split('(')
            .next()
            .unwrap()
            .split_whitespace()
            .last()
            .unwrap();
        connection
            .execute_batch(&format!("DROP TABLE {table}; {definition};"))
            .unwrap();
        if table == "schema_migrations" {
            connection
                .execute("INSERT INTO schema_migrations VALUES(1)", [])
                .unwrap();
        }
        let before = schema_snapshot(&connection);
        match StateStore::open(&database) {
            Err(error) => assert!(
                error.to_string().starts_with("STATE_SCHEMA_INVALID:"),
                "{definition}: {error}"
            ),
            Ok(_) => panic!("malformed schema accepted: {definition}"),
        }
        assert_eq!(schema_snapshot(&connection), before);
    }
}

#[test]
fn corrupt_sources_and_pins_never_escape_readback() {
    for (table, values) in [
        ("sources", "'unknown','user','main','repo'"),
        ("sources", "'apt',' ','main','repo'"),
        ("sources", "'apt','user','','repo'"),
        ("sources", "'apt','user','main',' '"),
        ("sources", "'apt','user','main','bad' || char(10)"),
        ("sources", "'apt',char(10),'main','repo'"),
        ("sources", "'apt','user',char(10),'repo'"),
        ("sources", "'apt','user','main',x'00'"),
        ("pins", "'APT','curl','8.0'"),
        ("pins", "'','curl','8.0'"),
        ("pins", "'apt',' ','8.0'"),
        ("pins", "'apt','curl',' '"),
        ("pins", "'apt','curl',x'00'"),
        ("pins", "'apt','curl',CAST(x'80' AS TEXT)"),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("corrupt.sqlite");
        let store = StateStore::open(&database).unwrap();
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection
            .execute_batch(&format!("INSERT INTO {table} VALUES({values})"))
            .unwrap();
        let error = if table == "sources" {
            store.sources().unwrap_err()
        } else {
            let (driver, name): (String, String) = connection
                .query_row("SELECT driver,name FROM pins", [], |row| {
                    Ok((row.get(0)?, row.get(1)?))
                })
                .unwrap();
            let error = store.pin_for(&driver, &name).unwrap_err();
            assert!(
                error.to_string().starts_with("STATE_RECORD_CORRUPT:"),
                "{values}: {error}"
            );
            store.pins().unwrap_err()
        };
        assert!(
            error.to_string().starts_with("STATE_RECORD_CORRUPT:"),
            "{values}: {error}"
        );
        assert!(std::error::Error::source(&error).is_some());
    }
}

#[test]
fn corrupt_existing_pin_is_not_treated_as_a_version_conflict() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("corrupt.sqlite");
    let store = StateStore::open(&database).unwrap();
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute("INSERT INTO pins VALUES('apt','curl',' ')", [])
        .unwrap();
    let package = PackageIdentity::new(ProviderId::new("apt").unwrap(), "curl", None).unwrap();
    let error = store.pin(&package, "8.0").unwrap_err();
    assert!(error.to_string().starts_with("STATE_RECORD_CORRUPT:"));
    let version: String = connection
        .query_row("SELECT version FROM pins", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, " ");
}

#[test]
fn overlapping_writers_report_busy_without_partial_writes_and_can_retry() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("contended.sqlite");
    let store = StateStore::open(&database).unwrap();
    let mut connection = rusqlite::Connection::open(&database).unwrap();
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .unwrap();
    transaction
        .execute("INSERT INTO pins VALUES('apt','curl','8.0')", [])
        .unwrap();

    // The first writer has written but cannot commit until the competing call
    // finishes. This proves real contention, not sequential connection access,
    // without scheduler assumptions or sleeps.
    let (store, error) = std::thread::spawn(move || {
        let package = PackageIdentity::new(ProviderId::new("apt").unwrap(), "curl", None).unwrap();
        let error = store.pin(&package, "8.0").unwrap_err();
        (store, error)
    })
    .join()
    .unwrap();
    assert!(error.to_string().starts_with("STATE_STORE_ERROR:"));
    let cause = std::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<rusqlite::Error>()
        .unwrap();
    assert_eq!(
        cause.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy)
    );
    // Timeout left no partial row. Only the still-uncommitted first writer exists.
    assert!(store.pins().unwrap().is_empty());
    transaction.commit().unwrap();
    let package = PackageIdentity::new(ProviderId::new("apt").unwrap(), "curl", None).unwrap();
    store.pin(&package, "8.0").unwrap();
    assert_eq!(store.pins().unwrap().len(), 1);
    assert_eq!(
        store.pin_for("apt", "curl").unwrap().unwrap().version,
        "8.0"
    );
}

#[test]
fn migration_history_corruption_and_preexisting_unversioned_schema_are_rejected() {
    for setup in [
        "DROP TABLE schema_migrations",
        "DELETE FROM schema_migrations",
        "PRAGMA ignore_check_constraints=ON; UPDATE schema_migrations SET version=2",
        "PRAGMA user_version=0",
        "DELETE FROM schema_migrations; PRAGMA user_version=0",
        "DROP TABLE schema_migrations; PRAGMA user_version=0",
        "PRAGMA user_version=-1",
        "DROP TABLE sources; CREATE VIEW sources AS SELECT 'apt' AS driver,'user' AS scope,'main' AS name,'repo' AS location",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("invalid.sqlite");
        drop(StateStore::open(&database).unwrap());
        let connection = rusqlite::Connection::open(&database).unwrap();
        connection.execute_batch(setup).unwrap();
        let before = schema_snapshot(&connection);
        match StateStore::open(&database) {
            Err(error) => assert!(
                error.to_string().starts_with("STATE_SCHEMA_INVALID:"),
                "{setup}: {error}"
            ),
            Ok(_) => panic!("invalid migration/schema accepted: {setup}"),
        }
        // Rejection must not repair/rewrite the database.
        assert_eq!(schema_snapshot(&connection), before);
    }
}

#[test]
fn pin_readback_preserves_the_public_constructor_contract() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("native.sqlite");
    let store = StateStore::open(&database).unwrap();
    // Pins use ProviderId, not the more restrictive source-driver allowlist.
    let package = PackageIdentity::new(
        ProviderId::new("custom-driver").unwrap(),
        "@scope/native",
        Some("1:2+rev".into()),
    )
    .unwrap();
    store.pin(&package, "1:2+rev").unwrap();
    assert_eq!(
        store
            .pin_for("custom-driver", "@scope/native")
            .unwrap()
            .unwrap()
            .version,
        "1:2+rev"
    );
    assert_eq!(store.pins().unwrap().len(), 1);
}
