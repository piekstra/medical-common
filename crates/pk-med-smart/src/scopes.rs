//! SMART scopes, in the v1 (`patient/Observation.read`) or v2
//! (`patient/Observation.rs`) spelling.

use std::fmt;

use crate::discovery::SmartConfiguration;

/// Which spelling of resource scopes a server takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeVersion {
    /// `patient/<Type>.read`.
    V1,
    /// `patient/<Type>.rs` (read and search).
    V2,
}

impl ScopeVersion {
    /// `V2` when the server lists `permission-v2`, else `V1`, which every SMART
    /// server accepts.
    pub fn for_server(config: &SmartConfiguration) -> Self {
        if config.has_capability("permission-v2") {
            ScopeVersion::V2
        } else {
            ScopeVersion::V1
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            ScopeVersion::V1 => "read",
            ScopeVersion::V2 => "rs",
        }
    }
}

/// An ordered set of scopes. Built up from the standalone patient launch, or
/// parsed whole from a string for a server whose spelling is its own.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScopeSet(Vec<String>);

impl ScopeSet {
    pub fn new() -> Self {
        ScopeSet(Vec::new())
    }

    /// A standalone patient launch: `openid fhirUser launch/patient
    /// offline_access`. Add the resource scopes with [`Self::read_types`] or
    /// [`Self::read_all`].
    pub fn standalone_patient() -> Self {
        ScopeSet::new()
            .with("openid")
            .with("fhirUser")
            .with("launch/patient")
            .with("offline_access")
    }

    /// `patient/<Type>.read` (v1) or `.rs` (v2) for each type: explicit, for a
    /// server that grants no wildcard.
    pub fn read_types(mut self, version: ScopeVersion, types: &[&str]) -> Self {
        for t in types {
            self = self.with(format!("patient/{t}.{}", version.suffix()));
        }
        self
    }

    /// `patient/*.read` (v1) or `patient/*.rs` (v2).
    pub fn read_all(self, version: ScopeVersion) -> Self {
        self.with(format!("patient/*.{}", version.suffix()))
    }

    /// Add a scope, once.
    pub fn with(mut self, scope: impl Into<String>) -> Self {
        let scope = scope.into();
        if !scope.is_empty() && !self.0.contains(&scope) {
            self.0.push(scope);
        }
        self
    }

    pub fn without(mut self, scope: &str) -> Self {
        self.0.retain(|s| s != scope);
        self
    }

    /// A space-separated scope string as a server or a configuration spells
    /// it, kept in its order, repeats dropped.
    pub fn parse(text: &str) -> Self {
        text.split_whitespace()
            .fold(ScopeSet::new(), |set, scope| set.with(scope))
    }

    pub fn contains(&self, scope: &str) -> bool {
        self.0.iter().any(|s| s == scope)
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for ScopeSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_patient_launch_in_both_spellings() {
        let v1 = ScopeSet::standalone_patient()
            .read_types(ScopeVersion::V1, &["Patient", "Observation"]);
        assert_eq!(
            v1.to_string(),
            "openid fhirUser launch/patient offline_access patient/Patient.read patient/Observation.read"
        );
        let v2 = ScopeSet::standalone_patient().read_all(ScopeVersion::V2);
        assert!(v2.contains("patient/*.rs"));
    }

    #[test]
    fn a_custom_string_keeps_its_order_and_drops_repeats() {
        let set = ScopeSet::parse("  launch/patient  openid patient/Coverage.read openid ");
        assert_eq!(
            set.to_string(),
            "launch/patient openid patient/Coverage.read"
        );
        assert_eq!(
            set.clone().without("openid").to_string(),
            "launch/patient patient/Coverage.read"
        );
        assert!(ScopeSet::parse("").is_empty());
    }
}
