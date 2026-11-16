//! Strict, side-effect-free configuration and bounded GitHub selection contracts.
//! Parsing never runs hooks, evaluates scripts, resolves networks, or installs files.
use neoget_core::{CoreError, Outcome};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

const MAX_CONFIG_BYTES: usize = 1024 * 1024;
const MAX_ENTRIES: usize = 1024;

/// Explicit input format; no content sniffing or fallback on parse errors.
#[derive(Clone, Copy)]
pub enum Format {
    Toml,
    Yaml,
}

/// Neoget bundle v1. Unknown fields, including nested fields, are errors.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Bundle {
    pub version: u32,
    #[serde(default, deserialize_with = "unique_map")]
    pub tools: BTreeMap<String, Tool>,
    #[serde(default, deserialize_with = "unique_strings")]
    pub aliases: BTreeMap<String, String>,
    #[serde(default, deserialize_with = "unique_map")]
    pub sources: BTreeMap<String, Source>,
    #[serde(default, deserialize_with = "unique_strings")]
    pub pins: BTreeMap<String, String>,
}

/// Provider-native tool identity. GitHub tools require an asset declaration.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    #[serde(deserialize_with = "strict_string")]
    pub provider: String,
    #[serde(deserialize_with = "strict_string")]
    pub package: String,
    #[serde(default, deserialize_with = "present_string")]
    pub version: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub asset: Option<Asset>,
    /// Published executable basename, not a path. Does not rename native packages.
    #[serde(default, deserialize_with = "present_string")]
    pub rename: Option<String>,
    #[serde(default, deserialize_with = "string_list")]
    pub aliases: Vec<String>,
}

/// Catalog declaration only; loading it performs no network access.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    #[serde(deserialize_with = "strict_string")]
    pub provider: String,
    #[serde(deserialize_with = "strict_string")]
    pub url: String,
}

/// Exactly one of `exact` or `template` selects a release asset.
/// Templates accept only `{version}`, `{os}`, and `{arch}` placeholders.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Asset {
    #[serde(default, deserialize_with = "present_string")]
    pub exact: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub template: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub member: Option<Member>,
}

/// Safe grep-like archive filter: exact relative path or literal basename.
/// No shell, regex, glob, or substring execution is supported.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Member {
    #[serde(default, deserialize_with = "present_string")]
    pub exact: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub basename: Option<String>,
}

// YAML's deserialize_string accepts non-string scalars (including null). Use
// deserialize_any and a string-only visitor so YAML and TOML share one contract.
struct StrictString(String);
impl<'de> Deserialize<'de> for StrictString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StringOnly;
        impl serde::de::Visitor<'_> for StringOnly {
            type Value = StrictString;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a string, not a coerced scalar or null")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(StrictString(value.to_owned()))
            }
            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(StrictString(value))
            }
        }
        deserializer.deserialize_any(StringOnly)
    }
}

fn strict_string<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    StrictString::deserialize(deserializer).map(|value| value.0)
}

fn present_string<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    strict_string(deserializer).map(Some)
}

fn string_list<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    Vec::<StrictString>::deserialize(deserializer)
        .map(|values| values.into_iter().map(|value| value.0).collect())
}

fn unique_strings<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, String>, D::Error> {
    unique_map::<D, StrictString>(deserializer)
        .map(|map| map.into_iter().map(|(key, value)| (key, value.0)).collect())
}

fn present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

// Serde's standard BTreeMap visitor overwrites duplicate YAML keys. A strict
// handwritten config must reject them, including aliases and nested tool names.
fn unique_map<'de, D, T>(deserializer: D) -> Result<BTreeMap<String, T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Unique<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Unique<T> {
        type Value = BTreeMap<String, T>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a bounded mapping with unique keys")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(
            self,
            mut access: M,
        ) -> Result<Self::Value, M::Error> {
            let mut map = BTreeMap::new();
            while let Some(StrictString(key)) = access.next_key::<StrictString>()? {
                if map.len() >= MAX_ENTRIES || map.contains_key(&key) {
                    return Err(serde::de::Error::custom(
                        "duplicate key or mapping exceeds 1024 entries",
                    ));
                }
                map.insert(key, access.next_value()?);
            }
            Ok(map)
        }
    }
    deserializer.deserialize_map(Unique(std::marker::PhantomData))
}

fn invalid(message: &'static str) -> CoreError {
    CoreError::provider_failure("CONFIG_INVALID", message)
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

fn text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.chars().any(char::is_control)
        && !value.chars().any(char::is_whitespace)
}

fn safe_relative(value: &str) -> bool {
    value.len() <= 512
        && !value.is_empty()
        && value.split('/').all(atom)
        && !value.contains('\\')
        && !value.contains(':')
}

fn parse<T: serde::de::DeserializeOwned>(input: &str, format: Format) -> Outcome<T> {
    if input.len() > MAX_CONFIG_BYTES {
        return Err(invalid("configuration exceeds 1 MiB"));
    }
    match format {
        Format::Toml => toml::from_str(input).map_err(|cause| {
            CoreError::provider_failure_with_source(
                "CONFIG_SYNTAX",
                "invalid TOML configuration (unknown fields are rejected)",
                cause,
            )
        }),
        Format::Yaml => serde_yaml_ng::from_str(input).map_err(|cause| {
            CoreError::provider_failure_with_source(
                "CONFIG_SYNTAX",
                "invalid YAML configuration (unknown fields are rejected)",
                cause,
            )
        }),
    }
}

/// Parse and validate a bundle. Rejects unsupported versions without fallback.
pub fn parse_bundle(input: &str, format: Format) -> Outcome<Bundle> {
    let bundle: Bundle = parse(input, format)?;
    if bundle.version != 1 {
        return Err(invalid("only bundle version 1 is supported"));
    }
    bundle.validate()?;
    Ok(bundle)
}

/// Focused Neoenv interchange envelope. Only tool declarations are accepted.
/// Other Neoenv domains (env, hooks, tasks, runtime, remote) are intentionally errors.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NeoenvTools {
    #[serde(deserialize_with = "unique_map")]
    tools: BTreeMap<String, Tool>,
}

/// Parse the tool-only Neoenv envelope using the same strict tool declarations.
/// This is not a full Neoenv configuration evaluator.
pub fn parse_neoenv_tools(input: &str, format: Format) -> Outcome<BTreeMap<String, Tool>> {
    let parsed: NeoenvTools = parse(input, format)?;
    let bundle = Bundle {
        version: 1,
        tools: parsed.tools,
        aliases: BTreeMap::new(),
        sources: BTreeMap::new(),
        pins: BTreeMap::new(),
    };
    bundle.validate()?;
    Ok(bundle.tools)
}

impl Bundle {
    /// Validate manually constructed bundles as well as parsed ones.
    /// Performs no I/O and rejects conflicts instead of merging declarations.
    pub fn validate(&self) -> Outcome<()> {
        if self.version != 1 {
            return Err(invalid("only bundle version 1 is supported"));
        }
        if self.tools.len() + self.aliases.len() + self.sources.len() + self.pins.len()
            > MAX_ENTRIES
        {
            return Err(invalid("configuration exceeds 1024 entries"));
        }
        let mut aliases = self.aliases.clone();
        for (name, tool) in &self.tools {
            if !atom(name) {
                return Err(invalid("invalid tool name"));
            }
            neoget_core::ProviderId::new(&tool.provider)?;
            if !text(&tool.package) || tool.version.as_ref().is_some_and(|v| !text(v)) {
                return Err(invalid("invalid package or version"));
            }
            if tool.rename.as_ref().is_some_and(|v| !executable_name(v)) {
                return Err(invalid("rename must be a portable executable basename"));
            }
            if tool.provider == "github" {
                if tool.rename.is_none() && !executable_name(name) {
                    return Err(invalid(
                        "GitHub tool name requires a safe executable name or rename",
                    ));
                }
                let parts: Vec<_> = tool.package.split('/').collect();
                if parts.len() != 2 || !parts.iter().all(|v| atom(v)) {
                    return Err(invalid("GitHub package must be owner/repository"));
                }
                tool.asset
                    .as_ref()
                    .ok_or_else(|| invalid("GitHub tools require asset selection"))?
                    .validate()?;
            } else if tool.asset.is_some() || tool.rename.is_some() {
                return Err(invalid(
                    "asset and rename are only supported for owned GitHub binaries",
                ));
            }
            if tool.aliases.len() > 64 {
                return Err(invalid("too many tool aliases"));
            }
            for alias in &tool.aliases {
                if aliases.insert(alias.clone(), name.clone()).is_some() {
                    return Err(invalid("duplicate alias"));
                }
            }
        }
        for (alias, target) in &aliases {
            if !atom(alias) || self.tools.contains_key(alias) || !self.tools.contains_key(target) {
                return Err(invalid(
                    "alias must uniquely name an existing tool; alias chains are not allowed",
                ));
            }
        }
        for (name, source) in &self.sources {
            neoget_core::ProviderId::new(&source.provider)?;
            if !atom(name)
                || !text(&source.url)
                || !source.url.starts_with("https://")
                || source.url[8..].is_empty()
                || source.url[8..].contains('@')
            {
                return Err(invalid(
                    "source requires a name and credential-free HTTPS URL",
                ));
            }
        }
        for (name, version) in &self.pins {
            if !self.tools.contains_key(name) || !text(version) {
                return Err(invalid(
                    "pin requires an existing canonical tool and exact version",
                ));
            }
            if self.tools[name]
                .version
                .as_ref()
                .is_some_and(|v| v != version)
            {
                return Err(invalid("pin conflicts with declared tool version"));
            }
        }
        Ok(())
    }

    /// Resolve canonical tool names or one-hop aliases; never guesses a provider.
    pub fn tool(&self, name: &str) -> Option<(&str, &Tool)> {
        self.tools
            .get_key_value(name)
            .map(|(k, v)| (k.as_str(), v))
            .or_else(|| {
                let target = self.aliases.get(name).or_else(|| {
                    self.tools
                        .iter()
                        .find(|(_, tool)| tool.aliases.iter().any(|alias| alias == name))
                        .map(|(name, _)| name)
                })?;
                self.tools
                    .get_key_value(target)
                    .map(|(k, v)| (k.as_str(), v))
            })
    }
}

fn executable_name(value: &str) -> bool {
    if !atom(value) || value.starts_with('-') || value.ends_with('.') {
        return false;
    }
    let stem = value.split('.').next().unwrap_or("").to_ascii_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !(stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

impl Asset {
    fn validate(&self) -> Outcome<()> {
        match (&self.exact, &self.template) {
            (Some(name), None) if atom(name) => {}
            (None, Some(template)) => {
                let expanded = template
                    .replace("{version}", "v1")
                    .replace("{os}", "linux")
                    .replace("{arch}", "x86_64");
                if !atom(&expanded) {
                    return Err(invalid(
                        "asset template contains unsafe text or unknown placeholders",
                    ));
                }
            }
            _ => return Err(invalid("asset requires exactly one exact name or template")),
        }
        if let Some(member) = &self.member {
            member.validate()?;
        }
        Ok(())
    }

    /// Select exactly one release filename. Bounded to 4096 candidates of 512 bytes.
    /// Zero or ambiguous matches fail; templates expand to an exact filename.
    pub fn select<'a>(
        &self,
        names: &'a [String],
        version: &str,
        os: &str,
        arch: &str,
    ) -> Outcome<&'a str> {
        self.validate()?;
        if ![version, os, arch].iter().all(|v| atom(v)) {
            return Err(invalid("unsafe template substitution"));
        }
        let expected = self.exact.clone().unwrap_or_else(|| {
            self.template
                .as_ref()
                .expect("validated template")
                .replace("{version}", version)
                .replace("{os}", os)
                .replace("{arch}", arch)
        });
        if !atom(&expected) {
            return Err(invalid("expanded asset filename exceeds bounds"));
        }
        select_one(names, |name| name == expected)
    }
}

fn select_one(names: &[String], matches: impl Fn(&str) -> bool) -> Outcome<&str> {
    if names.len() > 4096 || names.iter().any(|name| name.len() > 512) {
        return Err(invalid("selection input exceeds bounds"));
    }
    let mut found = names.iter().filter(|name| matches(name));
    let selected = found
        .next()
        .ok_or_else(|| invalid("selection matched no candidates"))?;
    if found.next().is_some() {
        return Err(invalid("selection is ambiguous"));
    }
    Ok(selected)
}

impl Member {
    fn validate(&self) -> Outcome<()> {
        match (&self.exact, &self.basename) {
            (Some(path), None) if safe_relative(path) => Ok(()),
            (None, Some(name)) if atom(name) => Ok(()),
            _ => Err(invalid(
                "member requires exactly one safe exact path or literal basename",
            )),
        }
    }

    /// Select one regular archive member before extraction. Callers must provide
    /// metadata for the complete archive and must not follow links when extracting.
    /// Rejects unsafe paths and non-regular entries even when they do not match.
    pub fn select<'a>(&self, entries: &'a [ArchiveEntry]) -> Outcome<&'a ArchiveEntry> {
        self.validate()?;
        if entries.len() > 4096
            || entries
                .iter()
                .any(|entry| !entry.regular || !safe_relative(&entry.path))
        {
            return Err(invalid(
                "archive contains unsafe, non-regular, or too many members",
            ));
        }
        let mut matches = entries.iter().filter(|entry| match &self.exact {
            Some(path) => entry.path == *path,
            None => entry.path.rsplit('/').next() == self.basename.as_deref(),
        });
        let selected = matches
            .next()
            .ok_or_else(|| invalid("member selection matched no candidates"))?;
        if matches.next().is_some() {
            return Err(invalid("member selection is ambiguous"));
        }
        Ok(selected)
    }
}

/// Pre-extraction metadata. Symlinks, hardlinks, directories, and devices are not regular.
#[derive(Clone, Debug)]
pub struct ArchiveEntry {
    pub path: String,
    pub regular: bool,
}

/// Pure root selection. Explicit root beats `NEOGET_ROOT`,
/// which beats the caller's platform-specific default. Relative paths use `cwd`.
/// Selection creates nothing and does not canonicalize through symlinks.
pub fn select_root(
    explicit: Option<&Path>,
    environment: Option<&Path>,
    default: &Path,
    cwd: &Path,
) -> Outcome<PathBuf> {
    let selected = explicit.or(environment).unwrap_or(default);
    if selected.as_os_str().is_empty() {
        return Err(invalid("root cannot be empty"));
    }
    Ok(if selected.is_absolute() {
        selected.to_owned()
    } else {
        cwd.join(selected)
    })
}
