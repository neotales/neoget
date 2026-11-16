//! Typed request, planning, authorization, and journal state for frontends.
use neoget_core::{PackageIdentity, ProviderId};
use rusqlite::OptionalExtension;
use sha2::Digest;

/// SQLite-backed source, exact-pin, and desired-plan snapshots; no package execution.
///
/// Paths are caller-selected. Relative paths use process cwd; creating and securing
/// the parent directory is the caller's responsibility (see [`Self::open`]).
///
/// The caller owns the handle; dropping it closes the connection. Use separate
/// handles for overlapping operations (a handle is not `Sync`). SQLite serializes
/// writers with a five-second busy timeout; call order across handles is unspecified.
/// Each mutation is atomic, but a sequence of calls is not a transaction.
///
/// Database failures return `STATE_STORE_ERROR` with a cause. Invalid/future
/// schemas return `STATE_SCHEMA_INVALID` / `STATE_SCHEMA_FUTURE` without migration.
/// Source/pin reads validate stored values and return `STATE_RECORD_CORRUPT`
/// rather than unchecked records. Plan reads verify their digest (`PLAN_CORRUPT`).
///
/// ```
/// use neoget_core::{PackageIdentity, ProviderId};
/// use neoget_engine::StateStore;
///
/// let directory = tempfile::tempdir()?; // A private directory for this example.
/// // The application creates/secures this directory, not StateStore.
/// let database = directory.path().join("state.sqlite");
/// let store = StateStore::open(&database)?;
/// let package = PackageIdentity::new(ProviderId::new("apt")?, "curl", None)?;
/// store.pin(&package, "8.0")?; // Repeating this exact pin is safe.
/// let conflict = store.pin(&package, "9.0").unwrap_err();
/// assert!(conflict.to_string().starts_with("PIN_CONFLICT:"));
/// drop(store); // Close before the application removes its state directory.
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct StateStore {
    connection: rusqlite::Connection,
}

/// A normalized source key and its metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    pub driver: String,
    pub scope: String,
    pub name: String,
    pub location: String,
}

/// A desired package intent captured in a persisted bundle plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundlePlan {
    pub digest: String,
    pub intents: Vec<PackageIdentity>,
}

/// A stored exact pin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pin {
    pub driver: String,
    pub name: String,
    pub version: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanEncoding {
    format: u32,
    intents: Vec<PlanIntent>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanIntent {
    driver: String,
    name: String,
    version: Option<String>,
}

impl StateStore {
    /// Opens the database at the caller-selected path and atomically applies migrations.
    ///
    /// Relative paths use process cwd; there is no implicit home directory. The caller
    /// must create and secure the parent directory first. Symlinks are not rejected
    /// or resolved by this API. Ordinary paths create a database file, never parent
    /// directories; SQLite's special `:memory:` path instead creates a private,
    /// transient database for this handle.
    /// An incompatible database is rejected with `STATE_SCHEMA_INVALID`, a newer
    /// version with `STATE_SCHEMA_FUTURE`. Contention/I/O returns `STATE_STORE_ERROR`;
    /// opening, like writes, uses a five-second SQLite busy timeout (not a whole-call
    /// deadline). Failed migrations leave existing schema/data unchanged.
    pub fn open(path: impl AsRef<std::path::Path>) -> neoget_core::Outcome<Self> {
        let mut connection = rusqlite::Connection::open(path)
            .map_err(|error| store_error(error, "could not open state database"))?;
        connection
            .busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|error| {
                store_error(error, "could not configure database contention timeout")
            })?;
        migrate(&mut connection)?;
        Ok(Self { connection })
    }

    /// Adds each validated source in one transaction; repeating an identical import is safe.
    /// Every entry is validated before any database change.
    pub fn import_sources(&mut self, sources: &[Source]) -> neoget_core::Outcome<()> {
        for source in sources {
            validate_source(source)?;
        }
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| store_error(error, "could not begin source import"))?;
        for source in sources {
            transaction.execute("INSERT INTO sources VALUES(?1,?2,?3,?4) ON CONFLICT(driver,scope,name) DO UPDATE SET location=excluded.location", [&source.driver, &source.scope, &source.name, &source.location])
                .map_err(|error| store_error(error, "could not import sources"))?;
        }
        transaction
            .commit()
            .map_err(|error| store_error(error, "could not commit source import"))
    }

    /// Returns validated source snapshots in stable key order.
    /// Invalid stored fields return `STATE_RECORD_CORRUPT`, never a partial list.
    pub fn sources(&self) -> neoget_core::Outcome<Vec<Source>> {
        let mut statement = self
            .connection
            .prepare("SELECT driver,scope,name,location FROM sources ORDER BY driver,scope,name")
            .map_err(|error| store_error(error, "could not read sources"))?;
        let rows = statement
            .query_map([], |row| Ok(read_source(row)))
            .map_err(|error| store_error(error, "could not read sources"))?;
        rows.map(|row| {
            let source = row
                .map_err(|error| store_error(error, "could not read sources"))?
                .map_err(record_error)?;
            validate_source(&source).map_err(record_error)?;
            Ok(source)
        })
        .collect()
    }

    /// Removes a source by its complete normalized key; repeating removal is safe.
    pub fn remove_source(&self, driver: &str, scope: &str, name: &str) -> neoget_core::Outcome<()> {
        self.connection
            .execute(
                "DELETE FROM sources WHERE driver=?1 AND scope=?2 AND name=?3",
                [driver, scope, name],
            )
            .map_err(|error| store_error(error, "could not remove source"))?;
        Ok(())
    }

    /// Updates only the location metadata of an existing supported source.
    pub fn update_source(
        &self,
        driver: &str,
        scope: &str,
        name: &str,
        location: &str,
    ) -> neoget_core::Outcome<()> {
        if !supported_source_driver(driver) {
            return Err(neoget_core::CoreError::Unsupported(
                "source update for this driver",
            ));
        }
        let source = Source {
            driver: driver.into(),
            scope: scope.into(),
            name: name.into(),
            location: location.into(),
        };
        validate_source(&source)?;
        self.connection
            .execute(
                "UPDATE sources SET location=?4 WHERE driver=?1 AND scope=?2 AND name=?3",
                [driver, scope, name, location],
            )
            .map_err(|error| store_error(error, "could not update source"))?;
        if self.connection.changes() == 0 {
            return Err(neoget_core::CoreError::provider_failure(
                "SOURCE_NOT_FOUND",
                "source key does not exist",
            ));
        }
        Ok(())
    }

    /// Atomically records an observed, exact package version; identical repeats are safe.
    /// A different requested/existing version returns `PIN_CONFLICT` without mutation.
    /// A corrupt existing pin returns `STATE_RECORD_CORRUPT`.
    pub fn pin(
        &self,
        identity: &PackageIdentity,
        observed_version: &str,
    ) -> neoget_core::Outcome<()> {
        if observed_version.trim().is_empty() {
            return Err(neoget_core::CoreError::InvalidIdentity(
                "pin version is empty",
            ));
        }
        if identity
            .exact_version()
            .is_some_and(|v| v != observed_version)
        {
            return Err(neoget_core::CoreError::provider_failure(
                "PIN_CONFLICT",
                "requested version differs from observed version",
            ));
        }
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| store_error(error, "could not begin pin transaction"))?;
        transaction
            .execute(
                "INSERT INTO pins VALUES(?1,?2,?3) ON CONFLICT(driver,name) DO NOTHING",
                [
                    identity.provider().as_str(),
                    identity.name(),
                    observed_version,
                ],
            )
            .map_err(|error| store_error(error, "could not save pin"))?;
        let existing = transaction
            .query_row(
                "SELECT driver,name,version FROM pins WHERE driver=?1 AND name=?2",
                [identity.provider().as_str(), identity.name()],
                |row| Ok(read_pin(row)),
            )
            .map_err(|error| store_error(error, "could not read pin"))?
            .map_err(record_error)?;
        validate_pin(&existing).map_err(record_error)?;
        if existing.version != observed_version {
            return Err(neoget_core::CoreError::provider_failure(
                "PIN_CONFLICT",
                "a different version is already pinned",
            ));
        }
        transaction
            .commit()
            .map_err(|error| store_error(error, "could not commit pin"))?;
        Ok(())
    }

    /// Removes a package pin; repeating removal is safe.
    pub fn unpin(&self, driver: &str, name: &str) -> neoget_core::Outcome<()> {
        self.connection
            .execute(
                "DELETE FROM pins WHERE driver=?1 AND name=?2",
                [driver, name],
            )
            .map_err(|error| store_error(error, "could not remove pin"))?;
        Ok(())
    }

    /// Lists validated pin snapshots in stable driver/name order.
    /// Invalid stored identities/versions return `STATE_RECORD_CORRUPT`, not a partial list.
    pub fn pins(&self) -> neoget_core::Outcome<Vec<Pin>> {
        let mut statement = self
            .connection
            .prepare("SELECT driver,name,version FROM pins ORDER BY driver,name")
            .map_err(|error| store_error(error, "could not read pins"))?;
        let rows = statement
            .query_map([], |row| Ok(read_pin(row)))
            .map_err(|error| store_error(error, "could not read pins"))?;
        rows.map(|row| {
            let pin = row
                .map_err(|error| store_error(error, "could not read pins"))?
                .map_err(record_error)?;
            validate_pin(&pin).map_err(record_error)?;
            Ok(pin)
        })
        .collect()
    }

    /// Looks up one pin, returning `None` when that provider/name is not pinned.
    /// A returned snapshot satisfies the public identity constructors; malformed
    /// stored fields return `STATE_RECORD_CORRUPT`.
    pub fn pin_for(&self, driver: &str, name: &str) -> neoget_core::Outcome<Option<Pin>> {
        self.connection
            .query_row(
                "SELECT driver,name,version FROM pins WHERE driver=?1 AND name=?2",
                [driver, name],
                |row| Ok(read_pin(row)),
            )
            .optional()
            .map_err(|error| store_error(error, "could not read pin"))?
            .map(|row| {
                let pin = row.map_err(record_error)?;
                validate_pin(&pin).map_err(record_error)?;
                Ok(pin)
            })
            .transpose()
    }

    /// Persists a canonical digest and normalized package intents, without executing them.
    pub fn save_bundle_plan(
        &self,
        intents: &[PackageIdentity],
    ) -> neoget_core::Outcome<BundlePlan> {
        let mut normalized = intents.to_vec();
        normalized.sort_by(|a, b| {
            (a.provider().as_str(), a.name(), a.exact_version()).cmp(&(
                b.provider().as_str(),
                b.name(),
                b.exact_version(),
            ))
        });
        for pair in normalized.windows(2) {
            if pair[0].provider() == pair[1].provider()
                && pair[0].name() == pair[1].name()
                && pair[0].exact_version() != pair[1].exact_version()
            {
                return Err(neoget_core::CoreError::provider_failure(
                    "PIN_CONFLICT",
                    "bundle contains conflicting exact versions",
                ));
            }
        }
        let encoding = PlanEncoding {
            format: 1,
            intents: normalized
                .iter()
                .map(|i| PlanIntent {
                    driver: i.provider().as_str().to_owned(),
                    name: i.name().to_owned(),
                    version: i.exact_version().map(str::to_owned),
                })
                .collect(),
        };
        let encoded = serde_json::to_vec(&encoding)
            .map_err(|error| store_error(error, "could not encode bundle plan"))?;
        let digest = format!("{:x}", sha2::Sha256::digest(&encoded));
        let transaction = self
            .connection
            .unchecked_transaction()
            .map_err(|error| store_error(error, "could not begin plan transaction"))?;
        let previous: Option<Vec<u8>> = transaction
            .query_row(
                "SELECT intents FROM bundle_plans WHERE digest=?1",
                [&digest],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| store_error(error, "could not inspect bundle plan"))?;
        if let Some(previous) = previous {
            if previous != encoded {
                return Err(neoget_core::CoreError::provider_failure(
                    "PLAN_DIGEST_CONFLICT",
                    "stored plan digest maps to different intent bytes",
                ));
            }
        } else {
            transaction
                .execute(
                    "INSERT INTO bundle_plans VALUES(?1,?2)",
                    rusqlite::params![digest, encoded],
                )
                .map_err(|error| store_error(error, "could not save bundle plan"))?;
        }
        transaction
            .commit()
            .map_err(|error| store_error(error, "could not commit bundle plan"))?;
        Ok(BundlePlan {
            digest,
            intents: normalized,
        })
    }

    /// Loads a plan by digest and verifies its stored bytes against the digest.
    pub fn load_bundle_plan(&self, digest: &str) -> neoget_core::Outcome<Option<BundlePlan>> {
        let encoded: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT intents FROM bundle_plans WHERE digest=?1",
                [digest],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| store_error(error, "could not load bundle plan"))?;
        let Some(encoded) = encoded else {
            return Ok(None);
        };
        if format!("{:x}", sha2::Sha256::digest(&encoded)) != digest {
            return Err(neoget_core::CoreError::provider_failure(
                "PLAN_CORRUPT",
                "stored plan digest does not match its contents",
            ));
        }
        let encoding: PlanEncoding = serde_json::from_slice(&encoded)
            .map_err(|error| store_error(error, "stored plan encoding is invalid"))?;
        if encoding.format != 1 {
            return Err(neoget_core::CoreError::provider_failure(
                "PLAN_FORMAT_UNSUPPORTED",
                "stored plan encoding version is unsupported",
            ));
        }
        let intents = encoding
            .intents
            .into_iter()
            .map(|i| PackageIdentity::new(ProviderId::new(i.driver)?, i.name, i.version))
            .collect::<neoget_core::Outcome<Vec<_>>>()?;
        Ok(Some(BundlePlan {
            digest: digest.to_owned(),
            intents,
        }))
    }
}

fn supported_source_driver(driver: &str) -> bool {
    matches!(
        driver,
        "winget" | "chocolatey" | "apt" | "pacman" | "dnf" | "yum" | "github" | "aqua"
    )
}

// Query callbacks wrap these decoders in an inner Result: SQL execution failures
// stay STATE_STORE_ERROR, while field decoding failures are STATE_RECORD_CORRUPT.
fn read_source(row: &rusqlite::Row<'_>) -> rusqlite::Result<Source> {
    Ok(Source {
        driver: row.get(0)?,
        scope: row.get(1)?,
        name: row.get(2)?,
        location: row.get(3)?,
    })
}

fn read_pin(row: &rusqlite::Row<'_>) -> rusqlite::Result<Pin> {
    Ok(Pin {
        driver: row.get(0)?,
        name: row.get(1)?,
        version: row.get(2)?,
    })
}

fn validate_pin(pin: &Pin) -> neoget_core::Outcome<()> {
    PackageIdentity::new(
        ProviderId::new(pin.driver.clone())?,
        &pin.name,
        Some(pin.version.clone()),
    )?;
    Ok(())
}

fn record_error(error: impl std::error::Error + Send + Sync + 'static) -> neoget_core::CoreError {
    neoget_core::CoreError::StateStore {
        code: "STATE_RECORD_CORRUPT",
        message: "stored source or pin violates its record contract",
        source: Box::new(error),
    }
}

fn schema_invalid() -> neoget_core::CoreError {
    neoget_core::CoreError::provider_failure(
        "STATE_SCHEMA_INVALID",
        "state schema or migration history is incompatible",
    )
}

// Column order matters: inserts use positional VALUES. Explicit NOT NULL matters
// even for text primary keys, which SQLite otherwise permits to contain NULL.
fn validate_table(
    connection: &rusqlite::Connection,
    table: &str,
    expected: &[(&str, &str, i64)],
) -> neoget_core::Outcome<()> {
    let is_table: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?1)",
            [table],
            |row| row.get(0),
        )
        .map_err(|error| store_error(error, "could not inspect state schema"))?;
    if !is_table {
        return Err(schema_invalid());
    }
    let mut statement = connection
        .prepare("SELECT name,type,\"notnull\",pk,hidden FROM pragma_table_xinfo(?1) ORDER BY cid")
        .map_err(|error| store_error(error, "could not inspect state schema"))?;
    let columns = statement
        .query_map([table], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })
        .map_err(|error| store_error(error, "could not inspect state schema"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| store_error(error, "could not inspect state schema"))?;
    if columns.len() != expected.len()
        || columns.iter().zip(expected).any(
            |((name, kind, not_null, primary_key, hidden), (want_name, want_kind, want_key))| {
                name != want_name
                    || !kind.eq_ignore_ascii_case(want_kind)
                    || !not_null
                    || primary_key != want_key
                    || *hidden != 0
            },
        )
    {
        return Err(schema_invalid());
    }

    // Check the actual uniqueness backing the key, not merely named columns.
    // Non-binary collations and partial/expression indexes change key semantics.
    let mut primary_key: Vec<_> = expected.iter().filter(|column| column.2 != 0).collect();
    primary_key.sort_by_key(|column| column.2);
    let key_names: Vec<_> = primary_key.iter().map(|column| column.0).collect();
    let mut statement = connection
        .prepare("SELECT name,origin,partial FROM pragma_index_list(?1) WHERE \"unique\"=1")
        .map_err(|error| store_error(error, "could not inspect state constraints"))?;
    let indexes = statement
        .query_map([table], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
            ))
        })
        .map_err(|error| store_error(error, "could not inspect state constraints"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| store_error(error, "could not inspect state constraints"))?;
    // INTEGER PRIMARY KEY uses the rowid and has no backing index.
    let mut has_primary_index = primary_key.len() == 1 && primary_key[0].1 == "INTEGER";
    for (index, origin, partial) in indexes {
        let mut statement = connection
            .prepare("SELECT name,coll,desc FROM pragma_index_xinfo(?1) WHERE key=1 ORDER BY seqno")
            .map_err(|error| store_error(error, "could not inspect state constraints"))?;
        let columns = statement
            .query_map([index], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            })
            .map_err(|error| store_error(error, "could not inspect state constraints"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| store_error(error, "could not inspect state constraints"))?;
        if partial
            || columns.len() != key_names.len()
            || columns
                .iter()
                .zip(&key_names)
                .any(|((name, collation, descending), expected)| {
                    name.as_deref() != Some(*expected) || collation != "BINARY" || *descending
                })
        {
            return Err(schema_invalid());
        }
        has_primary_index |= origin == "pk";
    }
    if !has_primary_index {
        return Err(schema_invalid());
    }
    Ok(())
}

fn migrate(connection: &mut rusqlite::Connection) -> neoget_core::Outcome<()> {
    let transaction = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| store_error(error, "could not begin schema migration"))?;
    let user_version: i64 = transaction
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| store_error(error, "could not inspect schema version"))?;
    if user_version > 1 {
        return Err(neoget_core::CoreError::provider_failure(
            "STATE_SCHEMA_FUTURE",
            "database schema is newer than this application",
        ));
    }
    if user_version < 0 {
        return Err(neoget_core::CoreError::provider_failure(
            "STATE_SCHEMA_INVALID",
            "database schema version is malformed",
        ));
    }
    let migration_table: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations')", [], |row| row.get(0),
    ).map_err(|error| store_error(error, "could not inspect migration history"))?;
    if migration_table {
        validate_table(
            &transaction,
            "schema_migrations",
            &[("version", "INTEGER", 1)],
        )?;
        let mut statement = transaction
            .prepare("SELECT version FROM schema_migrations ORDER BY version")
            .map_err(|error| store_error(error, "could not inspect migration history"))?;
        let versions = statement
            .query_map([], |row| row.get::<_, i64>(0))
            .map_err(|error| store_error(error, "could not inspect migration history"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| store_error(error, "could not inspect migration history"))?;
        if versions.iter().any(|version| *version != 1)
            || (user_version == 1 && versions.as_slice() != [1])
        {
            return Err(neoget_core::CoreError::provider_failure(
                "STATE_SCHEMA_INVALID",
                "migration history is malformed or incompatible",
            ));
        }
        if user_version == 0 && !versions.is_empty() {
            return Err(neoget_core::CoreError::provider_failure(
                "STATE_SCHEMA_INVALID",
                "migration history disagrees with schema version",
            ));
        }
    } else if user_version != 0 {
        return Err(neoget_core::CoreError::provider_failure(
            "STATE_SCHEMA_INVALID",
            "versioned database is missing migration history",
        ));
    }
    if user_version == 0 {
        let nonempty: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name NOT GLOB 'sqlite_*')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| store_error(error, "could not inspect initial state schema"))?;
        if nonempty {
            return Err(schema_invalid());
        }
        transaction.execute_batch(
            "CREATE TABLE schema_migrations(version INTEGER NOT NULL PRIMARY KEY CHECK(version=1));
             CREATE TABLE sources(driver TEXT NOT NULL,scope TEXT NOT NULL,name TEXT NOT NULL,location TEXT NOT NULL,PRIMARY KEY(driver,scope,name));
             CREATE TABLE pins(driver TEXT NOT NULL,name TEXT NOT NULL,version TEXT NOT NULL,PRIMARY KEY(driver,name));
             CREATE TABLE bundle_plans(digest TEXT NOT NULL PRIMARY KEY,intents BLOB NOT NULL);
             INSERT INTO schema_migrations(version) VALUES(1);
             PRAGMA user_version=1;",
        ).map_err(|error| store_error(error, "could not apply state schema migration"))?;
    } else {
        for (table, expected) in [
            (
                "sources",
                vec![
                    ("driver", "TEXT", 1),
                    ("scope", "TEXT", 2),
                    ("name", "TEXT", 3),
                    ("location", "TEXT", 0),
                ],
            ),
            (
                "pins",
                vec![
                    ("driver", "TEXT", 1),
                    ("name", "TEXT", 2),
                    ("version", "TEXT", 0),
                ],
            ),
            (
                "bundle_plans",
                vec![("digest", "TEXT", 1), ("intents", "BLOB", 0)],
            ),
        ] {
            validate_table(&transaction, table, &expected)?;
        }
    }
    transaction
        .commit()
        .map_err(|error| store_error(error, "could not commit schema migration"))
}
fn validate_source(source: &Source) -> neoget_core::Outcome<()> {
    if !supported_source_driver(&source.driver) {
        return Err(neoget_core::CoreError::Unsupported(
            "source import for this driver",
        ));
    }
    for value in [&source.scope, &source.name, &source.location] {
        if value.trim().is_empty() || value.chars().any(char::is_control) {
            return Err(neoget_core::CoreError::provider_failure(
                "SOURCE_INVALID",
                "source fields must be non-empty and contain no control characters",
            ));
        }
    }
    Ok(())
}
fn store_error(
    error: impl std::error::Error + Send + Sync + 'static,
    message: &'static str,
) -> neoget_core::CoreError {
    neoget_core::CoreError::StateStore {
        code: "STATE_STORE_ERROR",
        message,
        source: Box::new(error),
    }
}

/// User intent before resolution; package targets retain provider-native spelling.
///
/// Targets use `[provider:]package[@exact-version]`; scoped package names are
/// retained as written. Bare ambiguous names must be resolved explicitly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
    Search { query: String },
    Show { target: String },
    Install { target: String },
    List,
    Outdated,
    Upgrade { target: String },
    Uninstall { target: String },
    Refresh,
}

/// Supported v0 engine actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Search,
    Show,
    Install,
    List,
    Outdated,
    Upgrade,
    Uninstall,
    Refresh,
}

/// A resolved operation, with variants preventing target/action mismatches.
///
/// `Install` means ensure the requested version is installed; it never implies
/// an upgrade. `Refresh` refreshes metadata only and does not mutate installed
/// packages. Execution and persistence are not implemented in this crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Plan {
    Search { query: String },
    Show { package: PackageIdentity },
    Install { package: PackageIdentity },
    List,
    Outdated,
    Upgrade { package: PackageIdentity },
    Uninstall { package: PackageIdentity },
    Refresh,
}

/// Explicit authorization decision, kept separate from planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Authorization {
    Pending,
    Approved,
    Denied,
}

/// Future journal transition states; this crate does not persist or execute them yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalState {
    Planned,
    Authorized,
    Executing,
    Verified,
    Failed,
}

/// One state record for a future execution journal; no persistence is implemented.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalEntry {
    pub provider: ProviderId,
    pub state: JournalState,
    pub authorization: Authorization,
}
/// An owned-binary proposal only. No files are downloaded, extracted, or published.
#[derive(Debug, PartialEq)]
pub struct OwnedBinaryPlan {
    pub tool: String,
    pub asset: String,
    pub member: Option<String>,
    pub executable: String,
    pub destination: std::path::PathBuf,
}

/// Bounded release metadata supplied by a caller, not fetched by the planner.
pub struct OwnedBinaryRelease<'a> {
    pub assets: &'a [String],
    pub members: &'a [neoget_config::ArchiveEntry],
    pub version: &'a str,
    pub os: &'a str,
    pub arch: &'a str,
}

/// Resolve a configured GitHub tool/alias and exact asset/member metadata before
/// extraction. Rename changes only the proposed owned executable basename.
/// Destination is joined to the already resolved bin root, not process cwd.
/// Native packages are not eligible. This function performs no side effects.
///
/// ```
/// use neoget_config::{
///     parse_bundle, resolve_bin_root, ArchiveEntry, BinRootOverrides,
///     BinRootPlatform, Format, InstallScope,
/// };
/// use neoget_engine::{plan_owned_binary, OwnedBinaryRelease};
/// let bundle = parse_bundle(r#"
/// version = 1
/// [bin]
/// user = "tools/bin"
/// [tools.demo]
/// provider = "github"
/// package = "owner/demo"
/// rename = "demo-owned"
/// [tools.demo.asset]
/// exact = "demo.tar.gz"
/// [tools.demo.asset.member]
/// exact = "demo/bin/demo"
/// "#, Format::Toml)?;
/// let cwd = std::env::current_dir()?;
/// let root = resolve_bin_root(
///     InstallScope::User,
///     BinRootOverrides {
///         user: bundle.bin.user.as_deref().map(std::path::Path::new),
///         ..Default::default()
///     },
///     BinRootPlatform::Unix { home: None },
///     &cwd,
/// )?;
/// let assets = vec!["demo.tar.gz".to_owned()];
/// let members = vec![ArchiveEntry { path: "demo/bin/demo".into(), regular: true }];
/// let plan = plan_owned_binary(&bundle, "demo", OwnedBinaryRelease {
///     assets: &assets, members: &members, version: "1.0",
///     os: "linux", arch: "x86_64",
/// }, &root)?;
/// assert_eq!(plan.destination, root.path().join("demo-owned"));
/// // Only a proposal: no download, extraction, directory creation, or publication.
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn plan_owned_binary(
    bundle: &neoget_config::Bundle,
    name: &str,
    release: OwnedBinaryRelease<'_>,
    bin_root: &neoget_config::ResolvedBinRoot,
) -> neoget_core::Outcome<OwnedBinaryPlan> {
    let OwnedBinaryRelease {
        assets,
        members,
        version,
        os,
        arch,
    } = release;
    bundle.validate()?;
    let (canonical, tool) = bundle.tool(name).ok_or_else(|| {
        neoget_core::CoreError::provider_failure(
            "TOOL_NOT_FOUND",
            "tool or alias is not configured",
        )
    })?;
    if tool.provider != "github" {
        return Err(neoget_core::CoreError::Unsupported(
            "owned binary planning for native packages",
        ));
    }
    let pinned = bundle.pins.get(canonical).or(tool.version.as_ref());
    if pinned.is_some_and(|pin| pin != version) {
        return Err(neoget_core::CoreError::provider_failure(
            "VERSION_CONFLICT",
            "resolved version differs from configured exact version",
        ));
    }
    let selection = tool.asset.as_ref().ok_or_else(|| {
        neoget_core::CoreError::provider_failure(
            "CONFIG_INVALID",
            "GitHub tool requires asset selection",
        )
    })?;
    let asset = selection.select(assets, version, os, arch)?.to_owned();
    let member = selection
        .member
        .as_ref()
        .map(|filter| filter.select(members).map(|entry| entry.path.clone()))
        .transpose()?;
    let executable = tool.rename.as_deref().unwrap_or(canonical);
    Ok(OwnedBinaryPlan {
        tool: canonical.to_owned(),
        asset,
        member,
        executable: executable.to_owned(),
        destination: bin_root.path().join(executable),
    })
}
