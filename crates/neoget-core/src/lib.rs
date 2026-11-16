//! Shared, provider-neutral identity, capability, outcome, and error contracts.

use std::{error::Error, fmt};

/// Stable provider identifier, independent of a user's display label.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProviderId(String);

impl ProviderId {
    /// Creates an identifier containing lowercase ASCII letters, digits, `-`, or `_`.
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        if value.is_empty()
            || !value.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-_".contains(&byte)
            })
        {
            return Err(CoreError::InvalidIdentity("invalid provider id"));
        }
        Ok(Self(value))
    }

    /// Returns the canonical provider identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Validated provider-native package name and optional exact version.
///
/// Scoped names (for example `@scope/name`) are retained verbatim. Version text
/// is provider-native and is never normalized.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageIdentity {
    provider: ProviderId,
    name: String,
    exact_version: Option<String>,
}

impl PackageIdentity {
    /// Constructs an identity, rejecting an empty name or empty supplied version.
    pub fn new(
        provider: ProviderId,
        name: impl Into<String>,
        exact_version: Option<String>,
    ) -> Result<Self, CoreError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(CoreError::InvalidIdentity("package name is empty"));
        }
        if exact_version
            .as_ref()
            .is_some_and(|version| version.trim().is_empty())
        {
            return Err(CoreError::InvalidIdentity("exact version is empty"));
        }
        Ok(Self {
            provider,
            name,
            exact_version,
        })
    }

    /// Returns the provider that owns this exact package identity.
    pub fn provider(&self) -> &ProviderId {
        &self.provider
    }
    /// Returns the provider-native package name without normalization.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the provider-native exact version, if one was requested.
    pub fn exact_version(&self) -> Option<&str> {
        self.exact_version.as_deref()
    }
}

/// Operations a provider explicitly supports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Capability {
    Search,
}

/// Provider capability declaration, in stable declaration order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCapabilities {
    pub operations: Vec<Capability>,
}

/// A provider-neutral package result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageRecord {
    pub identity: PackageIdentity,
    pub summary: String,
}

/// Stable failure category and human-readable message, optionally retaining a cause.
#[derive(Debug)]
pub enum CoreError {
    InvalidIdentity(&'static str),
    ProviderFailure {
        code: &'static str,
        message: String,
        source: Option<Box<dyn Error + Send + Sync>>,
    },
    Unsupported(&'static str),
}

impl CoreError {
    /// Creates a provider failure with a stable code and message but no underlying cause.
    pub fn provider_failure(code: &'static str, message: impl Into<String>) -> Self {
        Self::ProviderFailure {
            code,
            message: message.into(),
            source: None,
        }
    }

    /// Creates a provider failure while retaining its underlying error cause.
    pub fn provider_failure_with_source(
        code: &'static str,
        message: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::ProviderFailure {
            code,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentity(message) => write!(f, "invalid identity: {message}"),
            Self::ProviderFailure { code, message, .. } => write!(f, "{code}: {message}"),
            Self::Unsupported(operation) => write!(f, "unsupported operation: {operation}"),
        }
    }
}

impl Error for CoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ProviderFailure {
                source: Some(source),
                ..
            } => Some(source.as_ref()),
            _ => None,
        }
    }
}

/// Result returned by provider and engine operations.
pub type Outcome<T> = Result<T, CoreError>;
