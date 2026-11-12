//! DICOM headers, our own: a Part 10 reader that stops at Pixel Data and
//! never decodes a pixel, DICOMDIR reading, and study and series summaries.
//! No DICOM library of anyone else's.
//!
//! - [`read_header`] reads the preamble and the file meta information
//!   (explicit VR little endian), then the data set in its transfer syntax:
//!   explicit or implicit VR little endian, explicit VR big endian (retired),
//!   deflated, or any compressed syntax (whose data set is explicit VR little
//!   endian). Sequences of defined and undefined length, and encapsulated
//!   fragments in a nested icon, are read or skipped. Reading stops at Pixel
//!   Data.
//! - [`Header`] has typed getters (`study_uid`, `modality`, `study_date` as
//!   `YYYY-MM-DD`, ...), [`Header::get`] for any text element's value and
//!   [`Header::element`] for any element. The patient getters are for the
//!   person's own terminal: `Debug` on a header, a data set or an element
//!   shows no value.
//! - [`read_dicomdir`] lists the files a DICOMDIR names, with their study
//!   and series.
//! - [`StudySummary::from_headers`] groups headers into studies and series.
//! - Feature `test-util`: [`synthetic::SyntheticStudy`] writes a synthetic
//!   study, with a DICOMDIR, for any repository's tests and dogfooding.
//!
//! Limits keep a hostile file from costing memory: a text or number value
//! over [`MAX_VALUE_BYTES`] is refused (a bulk one skipped), a header over
//! [`MAX_HEADER_BYTES`] before Pixel Data is refused, and sequences nest
//! [`MAX_DEPTH`] deep at most. Errors name offsets and tags, never a value.

pub mod dicomdir;
pub mod error;
pub mod header;
mod read;
pub mod summary;
#[cfg(feature = "test-util")]
pub mod synthetic;
pub mod tag;

pub use dicomdir::{dicomdir_entries, read_dicomdir, DicomDirEntry};
pub use error::DicomError;
pub use header::{iso_date, person_name, Dataset, Element, Header, Value};
pub use read::{is_dicom, read_header};
pub use summary::{SeriesSummary, StudySummary};
pub use tag::{tags, Tag, Vr};

/// The longest text or number value read: 16 MiB. A longer bulk value (OB,
/// OW, UN, ...) is skipped instead.
pub const MAX_VALUE_BYTES: u64 = 16 * 1024 * 1024;
/// The most value bytes a header holds before Pixel Data: 64 MiB.
pub const MAX_HEADER_BYTES: u64 = 64 * 1024 * 1024;
/// The deepest sequence nesting read.
pub const MAX_DEPTH: usize = 16;

/// The Media Storage Directory Storage SOP class: a DICOMDIR's, not an
/// image's.
pub const MEDIA_STORAGE_DIRECTORY: &str = "1.2.840.10008.1.3.10";

/// Transfer syntax UIDs (PS3.5 §10, PS3.6 Annex A).
pub mod transfer_syntax {
    pub const IMPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2";
    pub const EXPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2.1";
    pub const DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN: &str = "1.2.840.10008.1.2.1.99";
    pub const EXPLICIT_VR_BIG_ENDIAN: &str = "1.2.840.10008.1.2.2";
}

/// [`read_header`] on a file.
pub fn read_file(path: impl AsRef<std::path::Path>) -> Result<Header, DicomError> {
    let file = std::fs::File::open(path).map_err(DicomError::Io)?;
    read_header(file)
}

/// Whether the file at `path` begins as a DICOM file ([`is_dicom`]); `false`
/// for one that does not open.
pub fn is_dicom_file(path: impl AsRef<std::path::Path>) -> bool {
    use std::io::Read;
    let mut prefix = Vec::with_capacity(132);
    std::fs::File::open(path)
        .and_then(|f| f.take(132).read_to_end(&mut prefix))
        .is_ok_and(|_| is_dicom(&prefix))
}
