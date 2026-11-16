//! Typed request, planning, authorization, and journal state for frontends.
use neoget_core::{PackageIdentity, ProviderId};

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
}

/// Resolve a configured GitHub tool/alias and exact asset/member metadata before
/// extraction. Rename changes only the proposed owned executable basename.
/// Native packages are not eligible. This function performs no side effects.
pub fn plan_owned_binary(
    bundle: &neoget_config::Bundle,
    name: &str,
    assets: &[String],
    members: &[neoget_config::ArchiveEntry],
    version: &str,
    os: &str,
    arch: &str,
) -> neoget_core::Outcome<OwnedBinaryPlan> {
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
    })
}
