//! Why a file did not read. Messages name byte offsets and tags, never a
//! value: a value may be a patient's name.

use std::fmt;

use pk_cli_core::CliError;

/// Variants may be added: match with a `_` arm.
#[derive(Debug)]
#[non_exhaustive]
pub enum DicomError {
    /// No `DICM` marker after the preamble, and no file meta group at the
    /// start either.
    NotDicom,
    /// The file ends inside an element.
    Truncated {
        offset: u64,
    },
    /// The bytes at `offset` are not DICOM as the transfer syntax encodes it.
    Malformed {
        offset: u64,
        what: String,
    },
    /// Past a limit this reader keeps ([`crate::MAX_VALUE_BYTES`],
    /// [`crate::MAX_HEADER_BYTES`], [`crate::MAX_DEPTH`]).
    Limit {
        offset: u64,
        what: &'static str,
    },
    Io(std::io::Error),
}

impl fmt::Display for DicomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DicomError::NotDicom => {
                write!(f, "not a DICOM file: no DICM marker after the preamble")
            }
            DicomError::Truncated { offset } => {
                write!(
                    f,
                    "the DICOM file ends inside an element begun at byte {offset}"
                )
            }
            DicomError::Malformed { offset, what } => {
                write!(f, "malformed DICOM at byte {offset}: {what}")
            }
            DicomError::Limit { offset, what } => {
                write!(f, "refusing DICOM at byte {offset}: {what}")
            }
            DicomError::Io(e) => write!(f, "reading the DICOM file: {e}"),
        }
    }
}

/// The I/O error inside [`DicomError::Io`]; its message is in `Display`
/// too, for a CLI that prints one line.
impl std::error::Error for DicomError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            DicomError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for DicomError {
    fn from(e: std::io::Error) -> Self {
        DicomError::Io(e)
    }
}

/// [`CliError::Usage`] for a file that is not DICOM, the caller having
/// pointed at it; [`CliError::Other`] for one that is and does not read.
impl From<DicomError> for CliError {
    fn from(e: DicomError) -> Self {
        match e {
            DicomError::NotDicom => CliError::Usage(e.to_string()),
            _ => CliError::Other(e.to_string()),
        }
    }
}
