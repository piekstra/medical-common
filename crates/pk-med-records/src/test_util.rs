//! A synthetic stream, for consumers' tests (feature `test-util`): the
//! synthetic patient of `pk_med_fhir::test_util`, every resource type once or
//! more, Binary after the DocumentReference that names it.

use std::io::Write;

use pk_cli_core::CliError;
use pk_med_fhir::test_util::{synthetic_all, PATIENT_ID};

use crate::dto::{kind, relationship, Organization, PatientContext};
use crate::stream::{Header, RecordsWriter, Source, Trailer};

/// The synthetic organization's id.
pub const ORGANIZATION_ID: &str = "org-0001";

/// A header for the synthetic patient at a synthetic organization.
pub fn synthetic_header() -> Header {
    Header::new(
        Source::new("pk-med-records", env!("CARGO_PKG_VERSION")),
        Organization {
            id: ORGANIZATION_ID.into(),
            name: "Example Health".into(),
            fhir_base: "https://fhir.example.test/r4".into(),
            vendor: Some("synthetic".into()),
            location: Some("Exampleville, ZZ".into()),
            aliases: vec![],
        },
        PatientContext {
            id: PATIENT_ID.into(),
            organization: ORGANIZATION_ID.into(),
            label: Some("synthetic".into()),
            relationship: Some(relationship::SELF.into()),
            kind: Some(kind::PATIENT.into()),
            member_role: None,
            fhir_user: Some(format!("Patient/{PATIENT_ID}")),
            name: None,
            session: None,
        },
        "2020-01-03T00:00:00Z",
    )
}

/// Write the synthetic stream, complete, with no skips.
pub fn write_synthetic_stream(out: impl Write) -> Result<Trailer, CliError> {
    let mut writer = RecordsWriter::start(out, &synthetic_header())?;
    for resource in synthetic_all() {
        writer.resource(&resource)?;
    }
    Ok(writer.finish("2020-01-03T00:01:00Z", None)?.trailer)
}
