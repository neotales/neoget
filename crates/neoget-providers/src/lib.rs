//! Provider interface and deterministic in-memory fake; no OS package-manager adapters.
use neoget_core::{
    Capability, CoreError, Outcome, PackageRecord, ProviderCapabilities, ProviderId,
};

/// Provider contract consumed by the engine.
pub trait Provider {
    /// Returns this provider's stable identifier.
    fn id(&self) -> &ProviderId;
    /// Returns only operations that the implementation actually exposes.
    fn capabilities(&self) -> &ProviderCapabilities;
    /// Searches a stable-order catalog snapshot.
    ///
    /// An empty query matches every record. Search is read-only and repeatable.
    /// Failures use a stable code/message and may retain an underlying error source.
    fn search(&self, query: &str) -> Outcome<Vec<PackageRecord>>;
}

/// In-memory catalog used for contract tests and examples.
pub struct FakeProvider {
    id: ProviderId,
    capabilities: ProviderCapabilities,
    packages: Vec<PackageRecord>,
}

impl FakeProvider {
    /// Creates a deterministic catalog snapshot, rejecting records owned by another provider.
    pub fn new(id: ProviderId, packages: Vec<PackageRecord>) -> Result<Self, CoreError> {
        if packages
            .iter()
            .any(|package| package.identity.provider() != &id)
        {
            return Err(CoreError::InvalidIdentity(
                "catalog record belongs to a different provider",
            ));
        }
        Ok(Self {
            id,
            capabilities: ProviderCapabilities {
                operations: vec![Capability::Search],
            },
            packages,
        })
    }
}

impl Provider for FakeProvider {
    fn id(&self) -> &ProviderId {
        &self.id
    }
    fn capabilities(&self) -> &ProviderCapabilities {
        &self.capabilities
    }
    fn search(&self, query: &str) -> Outcome<Vec<PackageRecord>> {
        let query = query.to_ascii_lowercase();
        Ok(self
            .packages
            .iter()
            .filter(|item| item.identity.name().to_ascii_lowercase().contains(&query))
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn identity_search_and_provider_error_source_form_public_contract() {
        let provider_id = ProviderId::new("fake").unwrap();
        let package = PackageRecord {
            identity: neoget_core::PackageIdentity::new(
                provider_id.clone(),
                "@scope/tool",
                Some("v1-native".into()),
            )
            .unwrap(),
            summary: "Example tool".into(),
        };
        let provider = FakeProvider::new(provider_id, vec![package]).unwrap();
        assert_eq!(provider.search("scope").unwrap().len(), 1);
        assert_eq!(provider.search("").unwrap().len(), 1);

        let cause = std::io::Error::other("catalog unavailable");
        let error =
            CoreError::provider_failure_with_source("CATALOG_IO", "catalog lookup failed", cause);
        assert_eq!(error.to_string(), "CATALOG_IO: catalog lookup failed");
        assert_eq!(error.source().unwrap().to_string(), "catalog unavailable");
    }

    #[test]
    fn fake_provider_rejects_cross_provider_records_and_claims_search_only() {
        let owner = ProviderId::new("one").unwrap();
        let other = ProviderId::new("two").unwrap();
        let record = PackageRecord {
            identity: neoget_core::PackageIdentity::new(other, "pkg", None).unwrap(),
            summary: String::new(),
        };
        assert!(FakeProvider::new(owner, vec![record]).is_err());
        let provider = FakeProvider::new(ProviderId::new("one").unwrap(), vec![]).unwrap();
        assert_eq!(provider.capabilities().operations, vec![Capability::Search]);
    }
}
