//! Typed request, planning, authorization, and journal state for frontends.
use neoget_core::{PackageIdentity, ProviderId};
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

impl StateStore {
    /// Opens the database at the caller-selected path and atomically applies migrations.
    /// No platform home-directory assumptions are made.
    pub fn open(path: impl AsRef<std::path::Path>) -> neoget_core::Outcome<Self> {
        let connection = rusqlite::Connection::open(path)
            .map_err(|_| store_error("could not open state database"))?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys=ON;
                 CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY);
                 BEGIN IMMEDIATE;
                 CREATE TABLE IF NOT EXISTS sources(
                   driver TEXT NOT NULL, scope TEXT NOT NULL, name TEXT NOT NULL,
                   location TEXT NOT NULL, PRIMARY KEY(driver,scope,name));
                 CREATE TABLE IF NOT EXISTS pins(
                   driver TEXT NOT NULL, name TEXT NOT NULL, version TEXT NOT NULL,
                   PRIMARY KEY(driver,name));
                 CREATE TABLE IF NOT EXISTS bundle_plans(
                   digest TEXT PRIMARY KEY, intents TEXT NOT NULL);
                 INSERT OR IGNORE INTO schema_migrations(version) VALUES(1);
                 COMMIT;",
            )
            .map_err(|_| store_error("could not migrate state database"))?;
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
            .map_err(|_| store_error("could not begin source import"))?;
        for source in sources {
            transaction.execute("INSERT INTO sources VALUES(?1,?2,?3,?4) ON CONFLICT(driver,scope,name) DO UPDATE SET location=excluded.location", [&source.driver, &source.scope, &source.name, &source.location])
                .map_err(|_| store_error("could not import sources"))?;
        }
        transaction
            .commit()
            .map_err(|_| store_error("could not commit source import"))
    }

    /// Returns sources in stable key order.
    pub fn sources(&self) -> neoget_core::Outcome<Vec<Source>> {
        let mut statement = self
            .connection
            .prepare("SELECT driver,scope,name,location FROM sources ORDER BY driver,scope,name")
            .map_err(|_| store_error("could not read sources"))?;
        let rows = statement
            .query_map([], |row| {
                Ok(Source {
                    driver: row.get(0)?,
                    scope: row.get(1)?,
                    name: row.get(2)?,
                    location: row.get(3)?,
                })
            })
            .map_err(|_| store_error("could not read sources"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|_| store_error("could not read sources"))
    }

    /// Removes a source by its complete normalized key; repeating removal is safe.
    pub fn remove_source(&self, driver: &str, scope: &str, name: &str) -> neoget_core::Outcome<()> {
        self.connection
            .execute(
                "DELETE FROM sources WHERE driver=?1 AND scope=?2 AND name=?3",
                [driver, scope, name],
            )
            .map_err(|_| store_error("could not remove source"))?;
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
            .map_err(|_| store_error("could not update source"))?;
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
        let result = self
            .connection
            .execute(
                "INSERT INTO pins VALUES(?1,?2,?3) ON CONFLICT(driver,name) DO NOTHING",
                [
                    identity.provider().as_str(),
                    identity.name(),
                    observed_version,
                ],
            )
            .map_err(|_| store_error("could not save pin"))?;
        let existing: String = self
            .connection
            .query_row(
                "SELECT version FROM pins WHERE driver=?1 AND name=?2",
                [identity.provider().as_str(), identity.name()],
                |row| row.get(0),
            )
            .map_err(|_| store_error("could not read pin"))?;
        let _ = result;
        if existing != observed_version {
            return Err(neoget_core::CoreError::provider_failure(
                "PIN_CONFLICT",
                "a different version is already pinned",
            ));
        }
        Ok(())
    }

    /// Removes a package pin; repeating removal is safe.
    pub fn unpin(&self, driver: &str, name: &str) -> neoget_core::Outcome<()> {
        self.connection
            .execute(
                "DELETE FROM pins WHERE driver=?1 AND name=?2",
                [driver, name],
            )
            .map_err(|_| store_error("could not remove pin"))?;
        Ok(())
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
        let canonical = normalized
            .iter()
            .map(|i| {
                format!(
                    "{}:{}@{}",
                    i.provider(),
                    i.name(),
                    i.exact_version().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let digest = format!("{:x}", sha2::Sha256::digest(canonical.as_bytes()));
        let intents_json = normalized
            .iter()
            .map(|i| {
                format!(
                    "{}:{}@{}",
                    i.provider(),
                    i.name(),
                    i.exact_version().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.connection
            .execute(
                "INSERT OR IGNORE INTO bundle_plans VALUES(?1,?2)",
                [&digest, &intents_json],
            )
            .map_err(|_| store_error("could not save bundle plan"))?;
        Ok(BundlePlan {
            digest,
            intents: normalized,
        })
    }
}

fn supported_source_driver(driver: &str) -> bool {
    matches!(
        driver,
        "winget" | "chocolatey" | "apt" | "pacman" | "dnf" | "yum" | "github" | "aqua"
    )
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
fn store_error(message: &'static str) -> neoget_core::CoreError {
    neoget_core::CoreError::provider_failure("STATE_STORE_ERROR", message)
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
