//! The profile's DTOs: organizations, patient contexts, provenance and the
//! export summary. Every optional field is omitted when empty, so a field
//! added within `/v1` follows the family's additive carve-out.

use std::collections::BTreeMap;

use pk_cli_core::Paged;
use serde::{Deserialize, Serialize};

use crate::stream::{Finished, Header, Skip};
use pk_med_fhir::RawResource;

/// The profile's name, for `info`.
pub const PROFILE: &str = "health-records/v1";
pub const ORG_LIST_SCHEMA: &str = "org-list/v1";
pub const PATIENT_LIST_SCHEMA: &str = "patient-list/v1";
pub const PROVENANCE_SCHEMA: &str = "provenance/v1";
pub const EXPORT_SCHEMA: &str = "records-export/v1";

/// An organization a CLI can reach: a provider's FHIR server, or an
/// insurer's. Also the stream header's `organization`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    /// The CLI's own stable id for it (an Epic organization id, a slug).
    pub id: String,
    pub name: String,
    /// The FHIR base, normalized (`pk_med_fhir::normalize_base`). A source
    /// that is not a FHIR server (a member site) puts its site's base here.
    pub fhir_base: String,
    /// `epic`, `athenahealth`, `cerner`, `uhc`, ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    /// City and state, or a region, to tell same-named organizations apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    /// Other names it goes by (a brand, a former name), for `orgs search`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

/// A patient at a provider, or a member at an insurer, that a CLI holds a
/// sign-in for. Also the stream header's `patient`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatientContext {
    /// The FHIR `Patient.id` at the source.
    pub id: String,
    /// The [`Organization::id`] it is at.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub organization: String,
    /// The household's own name for the context (`--label`), never read from
    /// the record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// [`relationship::SELF`] or [`relationship::PROXY`] (a parent's access to
    /// a child's record).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relationship: Option<String>,
    /// [`kind::PATIENT`] at a provider, [`kind::MEMBER`] at an insurer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// A member's [`member_role::SUBSCRIBER`] or [`member_role::DEPENDENT`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_role: Option<String>,
    /// The signed-in user's `fhirUser` (`Patient/…`, `RelatedPerson/…`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fhir_user: Option<String>,
    /// The name the source shows, for `patients list` in the person's own
    /// terminal. Never in a fixture, a log or an error message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The CLI's keychain account for this sign-in (`smart/<org>/<patient>`),
    /// never the token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// `PatientContext.relationship` values.
pub mod relationship {
    pub const SELF: &str = "self";
    pub const PROXY: &str = "proxy";
}

/// `PatientContext.kind` values.
pub mod kind {
    pub const PATIENT: &str = "patient";
    pub const MEMBER: &str = "member";
}

/// `PatientContext.member_role` values.
pub mod member_role {
    pub const SUBSCRIBER: &str = "subscriber";
    pub const DEPENDENT: &str = "dependent";
}

/// `orgs list` and `orgs search`: `org-list/v1`.
pub fn org_list(items: Vec<Organization>) -> Paged<Organization> {
    Paged::new("org", items)
}

/// `patients list`: `patient-list/v1`.
pub fn patient_list(items: Vec<PatientContext>) -> Paged<PatientContext> {
    Paged::new("patient", items)
}

/// Where a filed record came from (`provenance/v1`), on every record a store
/// keeps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source: ProvenanceSource,
    /// One of [`method`]'s values.
    pub method: String,
    /// The resource at the source, for a record read from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_resource: Option<SourceResource>,
    /// When the source was read (the stream's `pulled_at`), RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    /// When the store filed it, RFC 3339.
    pub filed_at: String,
    /// The raw file in the records folder: the verbatim resource
    /// (`<Person>/Records/<source>/<Type>/<id>.json`), or the document as
    /// received.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_path: Option<String>,
    /// Who decided the person, date and type: one of [`classified_by`]'s
    /// values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classified_by: Option<String>,
    /// For a derived field: the document and the rule it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<DerivedFrom>,
    /// The program and version that filed it (`medrec 0.3.0`).
    pub filed_by: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceSource {
    /// One of [`source_kind`]'s values.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fhir_base: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceResource {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedFrom {
    pub document_id: String,
    pub rule_id: String,
}

/// `Provenance.method` values: how the record was obtained.
pub mod method {
    pub const FHIR_API: &str = "fhir-api";
    /// Mapped from a member or patient site, or from files downloaded there.
    pub const PORTAL: &str = "portal";
    pub const SCAN: &str = "scan";
    pub const FILE: &str = "file";
    pub const MEDIA: &str = "media";
    pub const MANUAL: &str = "manual";
    pub const HUISHOUDEN: &str = "huishouden";
}

/// `ProvenanceSource.kind` values: what kind of source it is.
pub mod source_kind {
    pub const FHIR: &str = "fhir";
    pub const PORTAL: &str = "portal";
    pub const SCAN: &str = "scan";
    pub const DRIVE: &str = "drive";
    pub const EMAIL: &str = "email";
    pub const MEDIA: &str = "media";
    pub const MANUAL: &str = "manual";
    pub const HUISHOUDEN: &str = "huishouden";
}

/// `Provenance.classified_by` values.
pub mod classified_by {
    /// The store's own rules.
    pub const MEDREC: &str = "medrec";
    pub const ORGANIZE_SCANS: &str = "organize-scans";
    /// On the command line or in review.
    pub const HAND: &str = "hand";
}

impl Provenance {
    /// The provenance of one resource read from a stream: the source from the
    /// header (its `method`, `fhir-api` when absent), the resource's type, id,
    /// version and last update, and `fetched_at` from `pulled_at`. The store
    /// adds `raw_path` and `classified_by`.
    pub fn from_stream(
        header: &Header,
        resource: &RawResource,
        filed_at: impl Into<String>,
        filed_by: impl Into<String>,
    ) -> Self {
        let method = header.method().to_string();
        let kind = if method == method::FHIR_API {
            source_kind::FHIR.to_string()
        } else {
            method.clone()
        };
        Provenance {
            source: ProvenanceSource {
                kind,
                cli: Some(header.source.cli.clone()),
                cli_version: Some(header.source.version.clone()),
                organization_id: Some(header.organization.id.clone()),
                fhir_base: Some(header.organization.fhir_base.clone()),
            },
            method,
            source_resource: Some(SourceResource {
                resource_type: resource.resource_type.clone(),
                id: resource.id.clone(),
                version_id: resource.version_id().map(str::to_string),
                last_updated: resource.last_updated().map(str::to_string),
            }),
            fetched_at: Some(header.pulled_at.clone()),
            filed_at: filed_at.into(),
            raw_path: None,
            classified_by: None,
            derived_from: None,
            filed_by: filed_by.into(),
        }
    }
}

/// What `records export -o <DIR>` wrote (`records-export/v1`), on stdout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordsExport {
    pub schema: String,
    /// The stream file written (`<DIR>/records.ndjson`).
    pub path: String,
    pub bytes: u64,
    /// The trailer's `complete`.
    pub complete: bool,
    /// Resources written, by type.
    pub counts: BTreeMap<String, u64>,
    /// What was left out, and why.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<Skip>,
}

impl RecordsExport {
    /// The summary of a stream written to `path`, from what
    /// [`crate::RecordsWriter::finish`] returned.
    pub fn new(path: impl Into<String>, finished: &Finished) -> Self {
        let trailer = &finished.trailer;
        RecordsExport {
            schema: EXPORT_SCHEMA.into(),
            path: path.into(),
            bytes: finished.bytes,
            complete: trailer.complete,
            counts: trailer.counts.clone(),
            skipped: trailer.skipped.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stream::Source;

    #[test]
    fn lists_carry_their_schemas() {
        assert_eq!(org_list(vec![]).schema, ORG_LIST_SCHEMA);
        assert_eq!(patient_list(vec![]).schema, PATIENT_LIST_SCHEMA);
    }

    #[test]
    fn empty_optionals_are_omitted() {
        let p = PatientContext {
            id: "pat-0001".into(),
            organization: String::new(),
            label: None,
            relationship: None,
            kind: None,
            member_role: None,
            fhir_user: None,
            name: None,
            session: None,
        };
        assert_eq!(serde_json::to_string(&p).unwrap(), r#"{"id":"pat-0001"}"#);
        let back: PatientContext = serde_json::from_str(r#"{"id":"pat-0001"}"#).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn provenance_from_a_stream_names_the_source_and_resource() {
        let header = Header::new(
            Source::new("mychart", "0.1.0"),
            Organization {
                id: "org-1".into(),
                name: "Example Health".into(),
                fhir_base: "https://fhir.example.test/r4".into(),
                vendor: Some("epic".into()),
                location: None,
                aliases: vec![],
            },
            PatientContext {
                id: "pat-0001".into(),
                organization: "org-1".into(),
                label: None,
                relationship: Some(relationship::SELF.into()),
                kind: None,
                member_role: None,
                fhir_user: None,
                name: None,
                session: None,
            },
            "2020-01-03T00:00:00Z",
        );
        let raw = RawResource::parse(
            r#"{"resourceType":"Observation","id":"obs-0001","meta":{"versionId":"2"}}"#,
        )
        .unwrap();
        let p = Provenance::from_stream(&header, &raw, "2020-01-04T00:00:00Z", "medrec 0.0.0");
        assert_eq!(
            (p.method.as_str(), p.source.kind.as_str()),
            ("fhir-api", "fhir")
        );
        let source = p.source_resource.as_ref().unwrap();
        assert_eq!(
            (source.resource_type.as_str(), source.id.as_str()),
            ("Observation", "obs-0001")
        );
        assert_eq!(source.version_id.as_deref(), Some("2"));
        let json = serde_json::to_value(&p).unwrap();
        assert_eq!(json["source_resource"]["type"], "Observation");
        assert!(json.get("raw_path").is_none());

        let mut portal = header.clone();
        portal.method = Some(method::PORTAL.into());
        let p = Provenance::from_stream(&portal, &raw, "t", "medrec");
        assert_eq!(
            (p.method.as_str(), p.source.kind.as_str()),
            ("portal", "portal")
        );
    }
}
