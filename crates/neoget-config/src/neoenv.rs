//! Native Neoenv tool-only schema, adapted from neoenv-config structured.rs.
//! Copyright (c) 2026 Neotales. MIT; see ../THIRD_PARTY_NOTICES.md.
//! These declarations are data only, not a resolver or conversion to a bundle.
use super::*;

fn format_version() -> u32 {
    1
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    #[serde(default = "format_version")]
    format_version: u32,
    #[serde(default)]
    tools: Vec<NeoenvTool>,
}

/// Native `[[tools]]` / YAML `tools: [...]` declaration.
/// Provider omission means `explicit-url` in Neoenv; the original spelling is
/// preserved here. Non-tool project domains are intentionally not accepted.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NeoenvTool {
    #[serde(deserialize_with = "strict_string")]
    pub id: String,
    #[serde(deserialize_with = "strict_string")]
    pub version: String,
    #[serde(default, deserialize_with = "present_string")]
    pub provider: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub url: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub sha256: Option<String>,
    #[serde(default, deserialize_with = "present")]
    pub github_release: Option<NeoenvGithubRelease>,
    #[serde(default, deserialize_with = "present")]
    pub gitlab_release: Option<NeoenvGitlabRelease>,
    #[serde(default, deserialize_with = "present")]
    pub aqua: Option<NeoenvAqua>,
    #[serde(default, deserialize_with = "present")]
    pub npm: Option<NeoenvNpm>,
    #[serde(default, deserialize_with = "present")]
    pub dotnet_tool: Option<NeoenvDotnetTool>,
    #[serde(default, deserialize_with = "present")]
    pub go_module: Option<NeoenvGoModule>,
    #[serde(default, deserialize_with = "present")]
    pub ruby_gems: Option<NeoenvRubyGems>,
    #[serde(default, deserialize_with = "string_list")]
    pub binaries: Vec<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub binpath: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub bindir: Option<String>,
    #[serde(default)]
    pub platforms: Vec<NeoenvPlatform>,
}

/// Native release selector. `grep` is carried as inert data only, exactly as
/// Neoenv's structured reader does. No regex is compiled or executed by Neoget.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NeoenvGithubRelease {
    #[serde(deserialize_with = "strict_string")]
    pub repository: String,
    #[serde(default, deserialize_with = "present_string")]
    pub asset: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub checksum_asset: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub grep: Option<String>,
}

/// Native per-platform override, preserved without host filtering.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NeoenvPlatform {
    #[serde(default, deserialize_with = "present_string")]
    pub os: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub arch: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub url: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub sha256: Option<String>,
    #[serde(default, deserialize_with = "optional_string_list")]
    pub binaries: Option<Vec<String>>,
    #[serde(default, deserialize_with = "present_string")]
    pub binpath: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub bindir: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub asset: Option<String>,
    #[serde(default, deserialize_with = "present_string")]
    pub grep: Option<String>,
}

fn optional_string_list<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error> {
    string_list(deserializer).map(Some)
}

fn gitlab_host() -> String {
    "gitlab.com".into()
}

/// Native GitLab declaration, parsed without claiming resolver availability.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NeoenvGitlabRelease {
    #[serde(deserialize_with = "strict_string")]
    pub namespace: String,
    #[serde(deserialize_with = "strict_string")]
    pub project: String,
    #[serde(default = "gitlab_host", deserialize_with = "strict_string")]
    pub host: String,
    #[serde(deserialize_with = "strict_string")]
    pub asset: String,
    #[serde(default, deserialize_with = "present_string")]
    pub grep: Option<String>,
}

macro_rules! native_strings {
    ($name:ident { $($field:ident),+ }) => {
        /// Native provider declaration, retained as inert data without resolution.
        #[derive(Clone, Debug, Deserialize, PartialEq)]
        #[serde(deny_unknown_fields)]
        pub struct $name { $(
            #[serde(deserialize_with = "strict_string")]
            pub $field: String,
        )+ }
    };
}
native_strings!(NeoenvAqua {
    endpoint,
    registry_revision,
    package,
    manifest_sha256
});
native_strings!(NeoenvNpm { endpoint, package });
native_strings!(NeoenvDotnetTool {
    source,
    endpoint,
    package
});
native_strings!(NeoenvGoModule {
    source,
    endpoint,
    module,
    go_sum
});
native_strings!(NeoenvRubyGems { endpoint, gem });

/// Parse the native Neoenv v1 tool-only profile in declaration order.
/// Accepts `format_version` (default 1) and `tools` (default empty list), rejects
/// other project domains and unknown nested fields. No scripts, templates,
/// regexes, imports, credentials, networks, or filesystem effects are evaluated.
/// Null/coerced strings and duplicate IDs are stricter than upstream parsing.
pub fn parse_neoenv_tools(input: &str, format: Format) -> Outcome<Vec<NeoenvTool>> {
    let document: Document = parse(input, format)?;
    if document.format_version != 1 {
        return Err(invalid("only Neoenv format_version 1 is supported"));
    }
    if document.tools.len() > MAX_ENTRIES {
        return Err(invalid("tool list exceeds 1024 entries"));
    }
    let mut ids = std::collections::BTreeSet::new();
    for tool in &document.tools {
        if !atom(&tool.id) || !text(&tool.version) || !ids.insert(&tool.id) {
            return Err(invalid("invalid or duplicate Neoenv tool ID or version"));
        }
        if tool.binaries.len() > 64
            || tool.platforms.len() > 64
            || tool.platforms.iter().any(|platform| {
                platform
                    .binaries
                    .as_ref()
                    .is_some_and(|binaries| binaries.len() > 64)
            })
        {
            return Err(invalid("tool exceeds binary or platform bounds"));
        }
    }
    Ok(document.tools)
}
