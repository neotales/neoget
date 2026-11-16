//! Safe structured parser detail for CLI presentation without source snippets.
use std::error::Error;

/// One-based source location and a redacted, actionable parser cause.
/// No config values, raw input lines, or filesystem paths are included.
pub struct ParseDiagnostic {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// Describe a recognized TOML/YAML cause without rendering its raw source.
/// Callers retain the original error chain and stable top-level failure code.
pub fn parse_diagnostic(cause: &(dyn Error + 'static), input: &str) -> Option<ParseDiagnostic> {
    if let Some(error) = cause.downcast_ref::<toml::de::Error>() {
        let offset = error.span().map_or(0, |span| span.start);
        let before = input.get(..offset).unwrap_or("");
        return Some(ParseDiagnostic {
            line: before.bytes().filter(|b| *b == b'\n').count() + 1,
            column: before.rsplit('\n').next().unwrap_or("").chars().count() + 1,
            message: safe_reason(error.message()),
        });
    }
    cause.downcast_ref::<serde_yaml_ng::Error>().map(|error| {
        let location = error.location();
        ParseDiagnostic {
            line: location.as_ref().map_or(1, |location| location.line()),
            column: location.as_ref().map_or(1, |location| location.column()),
            message: safe_reason(&error.to_string()),
        }
    })
}

fn safe_reason(message: &str) -> String {
    for label in ["unknown field", "missing field", "duplicate field"] {
        if let Some(rest) = message
            .split_once(&format!("{label} `"))
            .map(|(_, rest)| rest)
            && let Some((field, _)) = rest.split_once('`')
        {
            if field.len() <= 64
                && !field.is_empty()
                && field
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b))
            {
                return format!("{label} `{field}`");
            }
            return label.into();
        }
    }
    if message.contains("duplicate") {
        "duplicate configuration key".into()
    } else if message.contains("invalid type") {
        "invalid value type (strings must not be null, numeric, or boolean)".into()
    } else if message.contains("1024 entries") {
        "mapping exceeds 1024 entries".into()
    } else if message.contains("missing") {
        "missing required configuration component".into()
    } else if message.contains("expected") {
        "unexpected syntax or value; expected the declared configuration shape".into()
    } else {
        "invalid configuration syntax".into()
    }
}
