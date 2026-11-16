//! Typed request, planning, authorization, and journal state for frontends.
use neoget_core::{PackageIdentity, ProviderId};
use rusqlite::OptionalExtension;
use sha2::Digest;

/// SQLite-backed per-user state. Opening a store applies all pending schema migrations.
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
    /// No platform home-directory assumptions are made.
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

    /// Returns sources in stable key order.
    pub fn sources(&self) -> neoget_core::Outcome<Vec<Source>> {
        let mut statement = self
            .connection
            .prepare("SELECT driver,scope,name,location FROM sources ORDER BY driver,scope,name")
            .map_err(|error| store_error(error, "could not read sources"))?;
        let rows = statement
            .query_map([], |row| {
                Ok(Source {
                    driver: row.get(0)?,
                    scope: row.get(1)?,
                    name: row.get(2)?,
                    location: row.get(3)?,
                })
            })
            .map_err(|error| store_error(error, "could not read sources"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| store_error(error, "could not read sources"))
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

    /// Records an observed, exact package version. A conflicting existing pin fails.
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
        let existing: String = transaction
            .query_row(
                "SELECT version FROM pins WHERE driver=?1 AND name=?2",
                [identity.provider().as_str(), identity.name()],
                |row| row.get(0),
            )
            .map_err(|error| store_error(error, "could not read pin"))?;
        if existing != observed_version {
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

    /// Lists stored pins in stable driver/name order.
    pub fn pins(&self) -> neoget_core::Outcome<Vec<Pin>> {
        let mut statement = self
            .connection
            .prepare("SELECT driver,name,version FROM pins ORDER BY driver,name")
            .map_err(|error| store_error(error, "could not read pins"))?;
        let rows = statement
            .query_map([], |row| {
                Ok(Pin {
                    driver: row.get(0)?,
                    name: row.get(1)?,
                    version: row.get(2)?,
                })
            })
            .map_err(|error| store_error(error, "could not read pins"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| store_error(error, "could not read pins"))
    }

    /// Looks up one pin, returning `None` when that provider/name is not pinned.
    pub fn pin_for(&self, driver: &str, name: &str) -> neoget_core::Outcome<Option<Pin>> {
        self.connection
            .query_row(
                "SELECT driver,name,version FROM pins WHERE driver=?1 AND name=?2",
                [driver, name],
                |row| {
                    Ok(Pin {
                        driver: row.get(0)?,
                        name: row.get(1)?,
                        version: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(|error| store_error(error, "could not read pin"))
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
        let columns = {
            let mut statement = transaction
                .prepare("PRAGMA table_info(schema_migrations)")
                .map_err(|error| store_error(error, "could not inspect migration history"))?;
            statement
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|error| store_error(error, "could not inspect migration history"))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| store_error(error, "could not inspect migration history"))?
        };
        if columns != ["version"] {
            return Err(neoget_core::CoreError::provider_failure(
                "STATE_SCHEMA_INVALID",
                "migration history has an incompatible shape",
            ));
        }
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
            ("sources", vec!["driver", "scope", "name", "location"]),
            ("pins", vec!["driver", "name", "version"]),
            ("bundle_plans", vec!["digest", "intents"]),
        ] {
            let mut statement = transaction
                .prepare(&format!("PRAGMA table_info({table})"))
                .map_err(|error| store_error(error, "could not inspect state schema"))?;
            let actual = statement
                .query_map([], |row| row.get::<_, String>(1))
                .map_err(|error| store_error(error, "could not inspect state schema"))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| store_error(error, "could not inspect state schema"))?;
            if actual != expected {
                return Err(neoget_core::CoreError::provider_failure(
                    "STATE_SCHEMA_INVALID",
                    "state table has an incompatible shape",
                ));
            }
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
