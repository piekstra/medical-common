//! The `health-records/v1` profile: what every provider and insurer CLI of the
//! family emits, and what the records store reads. `DESIGN.md` at the
//! repository's root is the specification; this crate is its types.
//!
//! - [`dto`]: [`Organization`] (`org-list/v1`), [`PatientContext`]
//!   (`patient-list/v1`), [`Provenance`] (`provenance/v1`) and
//!   [`RecordsExport`] (`records-export/v1`).
//! - [`stream`]: the `records-stream/v1` NDJSON stream, its
//!   [`RecordsWriter`] and [`RecordsReader`].
//! - [`export`]: `records export -o <DIR>`'s file.
//!
//! Resources travel as [`RawResource`]: the server's own JSON text, so a
//! decimal keeps its spelling from source to store.

pub mod dto;
pub mod export;
pub mod stream;
#[cfg(feature = "test-util")]
pub mod test_util;

pub use dto::{
    org_list, patient_list, Organization, PatientContext, Provenance, ProvenanceSource,
    RecordsExport, SourceResource, PROFILE,
};
pub use export::{create_export_file, ExportFile, EXPORT_FILE};
pub use pk_med_fhir::RawResource;
pub use stream::{
    Finished, Header, Item, RecordsReader, RecordsWriter, Skip, Source, StreamError, Trailer,
    TrailerError, END_SCHEMA, MAX_LINE_BYTES, STREAM_SCHEMA,
};
