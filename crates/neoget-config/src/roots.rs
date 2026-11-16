//! Pure destination resolution, independent of process environment and filesystem.
use super::*;

/// Optional bundle bin roots, scoped explicitly rather than merged.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BinDirectories {
    #[serde(default, deserialize_with = "present_string")]
    pub user: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub global: Option<String>,
}

/// Scope chooses a destination only; global scope grants no authorization.
#[derive(Clone, Copy)]
pub enum InstallScope {
    User,
    Global,
}

/// Precedence inputs: explicit argument > environment > selected-scope config.
/// An explicitly empty path fails, rather than falling back.
#[derive(Default)]
pub struct BinRootOverrides<'a> {
    pub explicit: Option<&'a Path>,
    pub environment: Option<&'a Path>,
    pub user: Option<&'a Path>,
    pub global: Option<&'a Path>,
}

/// Injected platform bases; no environment is read by the resolver.
/// Unix covers Linux and macOS. Windows requires the appropriate per-scope base.
pub enum BinRootPlatform<'a> {
    Unix {
        home: Option<&'a Path>,
    },
    Windows {
        local_app_data: Option<&'a Path>,
        program_data: Option<&'a Path>,
    },
}

/// An absolute, lexically resolved bin root. Selection creates no directories
/// and does not promise symlink containment, permissions, or publish readiness.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedBinRoot(PathBuf);
impl ResolvedBinRoot {
    /// Absolute selected root; publication must perform its own no-follow checks.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// Resolve an owned-binary destination without filesystem access.
/// Precedence is explicit > environment > user/global config > platform default.
/// Unix defaults: user `HOME/.local/bin`, global `/usr/local/bin`.
/// Windows defaults: user `LOCALAPPDATA/Neoget/bin`, global `PROGRAMDATA/Neoget/bin`.
/// Relative paths use the supplied absolute `cwd`; missing required platform
/// bases, empty/control-containing paths, or a nonabsolute cwd fail.
pub fn resolve_bin_root(
    scope: InstallScope,
    overrides: BinRootOverrides<'_>,
    platform: BinRootPlatform<'_>,
    cwd: &Path,
) -> Outcome<ResolvedBinRoot> {
    if !cwd.is_absolute() {
        return Err(invalid(
            "bin root resolution requires an absolute working directory",
        ));
    }
    let configured = match scope {
        InstallScope::User => overrides.user,
        InstallScope::Global => overrides.global,
    };
    let selected = if let Some(path) = overrides.explicit.or(overrides.environment).or(configured) {
        path.to_owned()
    } else {
        match (scope, platform) {
            (InstallScope::User, BinRootPlatform::Unix { home }) => {
                required_base(home)?.join(".local/bin")
            }
            (InstallScope::Global, BinRootPlatform::Unix { .. }) => PathBuf::from("/usr/local/bin"),
            (InstallScope::User, BinRootPlatform::Windows { local_app_data, .. }) => {
                required_base(local_app_data)?.join("Neoget/bin")
            }
            (InstallScope::Global, BinRootPlatform::Windows { program_data, .. }) => {
                required_base(program_data)?.join("Neoget/bin")
            }
        }
    };
    if selected.as_os_str().is_empty()
        || selected.as_os_str().len() > 4096
        || selected.to_string_lossy().chars().any(char::is_control)
    {
        return Err(invalid("invalid bin root path"));
    }
    Ok(ResolvedBinRoot(if selected.is_absolute() {
        selected
    } else {
        cwd.join(selected)
    }))
}

fn required_base(base: Option<&Path>) -> Outcome<&Path> {
    base.filter(|base| !base.as_os_str().is_empty())
        .ok_or_else(|| invalid("bin root requires a platform base or explicit override"))
}
