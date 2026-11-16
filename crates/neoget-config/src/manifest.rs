//! Declarative, side-effect-free cross-platform package manifests.
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::{Format, parse};
use neoget_core::{CoreError, Outcome, ProviderId};

const MAX_MANIFEST_BYTES: usize = 1024 * 1024;

fn invalid(message: &'static str) -> CoreError {
    CoreError::provider_failure("MANIFEST_INVALID", message)
}

/// Version 1 package manifest. Map keys are stable local package identifiers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub version: u32,
    pub packages: BTreeMap<String, ManifestPackage>,
    pub roots: Vec<String>,
}

/// Provider-qualified package selection and optional host OS allow-list.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ManifestPackage {
    pub provider: String,
    pub package: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub platforms: Vec<Platform>,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// Supported host operating-system selectors.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Linux,
    Macos,
    Windows,
}

/// Host OS used for deterministic manifest resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPlatform {
    Linux,
    Macos,
    Windows,
}

impl From<Platform> for HostPlatform {
    fn from(value: Platform) -> Self {
        match value {
            Platform::Linux => Self::Linux,
            Platform::Macos => Self::Macos,
            Platform::Windows => Self::Windows,
        }
    }
}

/// Normalized lock-shaped selection, ordered by package identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedManifest {
    pub version: u32,
    pub host: HostPlatform,
    pub packages: BTreeMap<String, ResolvedPackage>,
    pub roots: Vec<String>,
}

/// One selected native identity and its already-resolved manifest dependencies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPackage {
    pub provider: String,
    pub package: String,
    pub version: Option<String>,
    pub dependencies: Vec<String>,
}

/// Parse a strict TOML or YAML manifest, rejecting unsupported schema versions.
pub fn parse_manifest(input: &str, format: Format) -> Outcome<Manifest> {
    if input.len() > MAX_MANIFEST_BYTES {
        return Err(invalid("manifest exceeds 1 MiB"));
    }
    let manifest: Manifest = parse(input, format)?;
    manifest.validate()?;
    Ok(manifest)
}

impl Manifest {
    /// Validate package identities, selectors, roots, and references without I/O.
    pub fn validate(&self) -> Outcome<()> {
        if self.version != 1 {
            return Err(invalid("only manifest version 1 is supported"));
        }
        if self.packages.is_empty() || self.packages.len() > 1024 || self.roots.is_empty() {
            return Err(invalid(
                "manifest requires bounded packages and at least one root",
            ));
        }
        for (id, package) in &self.packages {
            if !atom(id)
                || !atom(&package.package)
                || package.version.as_ref().is_some_and(|v| !atom(v))
            {
                return Err(invalid(
                    "invalid package identifier, name, or exact version",
                ));
            }
            ProviderId::new(&package.provider)?;
            if package.platforms.iter().collect::<BTreeSet<_>>().len() != package.platforms.len() {
                return Err(invalid("duplicate platform selector"));
            }
            if package.dependencies.iter().collect::<BTreeSet<_>>().len()
                != package.dependencies.len()
            {
                return Err(invalid("duplicate dependency"));
            }
            for dependency in &package.dependencies {
                if !atom(dependency) || !self.packages.contains_key(dependency) {
                    return Err(invalid("dependency must reference a declared package"));
                }
            }
        }
        if self.roots.iter().collect::<BTreeSet<_>>().len() != self.roots.len()
            || self
                .roots
                .iter()
                .any(|root| !self.packages.contains_key(root))
        {
            return Err(invalid("roots must be unique declared package identifiers"));
        }
        Ok(())
    }

    /// Select the dependency closure for a supported host without executing anything.
    pub fn resolve(&self, host: HostPlatform) -> Outcome<ResolvedManifest> {
        self.validate()?;
        let mut selected = BTreeSet::new();
        let mut visiting = BTreeSet::new();
        let mut done = BTreeSet::new();
        let mut identities = BTreeMap::<(String, String), Option<String>>::new();
        for root in &self.roots {
            visit(
                self,
                root,
                host,
                true,
                &mut visiting,
                &mut done,
                &mut selected,
                &mut identities,
            )?;
        }
        let packages = selected
            .into_iter()
            .map(|id| {
                let package = &self.packages[&id];
                (
                    id,
                    ResolvedPackage {
                        provider: package.provider.clone(),
                        package: package.package.clone(),
                        version: package.version.clone(),
                        dependencies: package
                            .dependencies
                            .iter()
                            .filter(|dep| done.contains(*dep))
                            .cloned()
                            .collect(),
                    },
                )
            })
            .collect();
        Ok(ResolvedManifest {
            version: 1,
            host,
            packages,
            roots: self.roots.clone(),
        })
    }
}

#[allow(clippy::too_many_arguments)] // Resolution state is explicit and shared across recursion.
fn visit(
    manifest: &Manifest,
    id: &str,
    host: HostPlatform,
    required: bool,
    visiting: &mut BTreeSet<String>,
    done: &mut BTreeSet<String>,
    selected: &mut BTreeSet<String>,
    identities: &mut BTreeMap<(String, String), Option<String>>,
) -> Outcome<()> {
    if done.contains(id) {
        return Ok(());
    }
    if !visiting.insert(id.to_owned()) {
        return Err(invalid("dependency cycle detected"));
    }
    let item = &manifest.packages[id];
    if !item.platforms.is_empty()
        && !item
            .platforms
            .iter()
            .any(|p| HostPlatform::from(*p) == host)
    {
        visiting.remove(id);
        return if required {
            Err(invalid("selected root is unsupported on this host"))
        } else {
            Ok(())
        };
    }
    let identity = (item.provider.clone(), item.package.clone());
    if identities
        .get(&identity)
        .is_some_and(|version| version != &item.version)
    {
        return Err(invalid(
            "conflicting versions selected for provider package",
        ));
    }
    identities.insert(identity, item.version.clone());
    for dependency in &item.dependencies {
        visit(
            manifest, dependency, host, false, visiting, done, selected, identities,
        )?;
    }
    visiting.remove(id);
    done.insert(id.to_owned());
    selected.insert(id.to_owned());
    Ok(())
}

fn atom(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        && value != "."
        && value != ".."
}
