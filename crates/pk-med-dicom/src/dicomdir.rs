//! A DICOMDIR: the index of a DICOM disc or folder (PS3.10 §8, PS3.3 F.5).

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use crate::error::DicomError;
use crate::header::{Dataset, Header};
use crate::tag::tags;

/// One file a DICOMDIR names, with the study and series it sits under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DicomDirEntry {
    /// The file, under the DICOMDIR's directory.
    pub path: PathBuf,
    /// `IMAGE`, `SR DOCUMENT`, `ENCAP DOC`, ...
    pub record_type: String,
    pub study_uid: Option<String>,
    pub series_uid: Option<String>,
    pub sop_uid: Option<String>,
    /// The series record's modality.
    pub modality: Option<String>,
}

/// Read the DICOMDIR at `path` and list every file it names, in its
/// records' order. Paths are under the DICOMDIR's own directory; a
/// Referenced File ID that would climb out of it is refused.
pub fn read_dicomdir(path: impl AsRef<Path>) -> Result<Vec<DicomDirEntry>, DicomError> {
    let path = path.as_ref();
    let file = std::fs::File::open(path).map_err(DicomError::Io)?;
    let header = crate::read_header(file)?;
    let root = path.parent().unwrap_or_else(|| Path::new("."));
    dicomdir_entries(&header, root)
}

#[derive(Clone, Default)]
struct Context {
    study_uid: Option<String>,
    series_uid: Option<String>,
    modality: Option<String>,
}

impl Context {
    fn enter(&self, record: &Dataset) -> Context {
        match record_type(record).as_str() {
            "PATIENT" => Context::default(),
            "STUDY" => Context {
                study_uid: record.string(tags::STUDY_INSTANCE_UID),
                ..Context::default()
            },
            "SERIES" => Context {
                study_uid: self.study_uid.clone(),
                series_uid: record.string(tags::SERIES_INSTANCE_UID),
                modality: record.string(tags::MODALITY),
            },
            _ => self.clone(),
        }
    }
}

fn record_type(record: &Dataset) -> String {
    record
        .string(tags::RECORD_TYPE)
        .unwrap_or_default()
        .to_ascii_uppercase()
}

/// The files a DICOMDIR header names, under `root`. Records are walked by
/// their offsets (first root record, next, lower level) when the header has
/// them, else in sequence order. Records marked not in use are left out.
pub fn dicomdir_entries(header: &Header, root: &Path) -> Result<Vec<DicomDirEntry>, DicomError> {
    let Some(sequence) = header.dataset().get(tags::DIRECTORY_RECORD_SEQUENCE) else {
        return Err(DicomError::Malformed {
            offset: 0,
            what: "not a DICOMDIR: no Directory Record Sequence".into(),
        });
    };
    let records = sequence.items();
    let by_offset: HashMap<u64, usize> = records
        .iter()
        .enumerate()
        .map(|(i, r)| (r.offset(), i))
        .collect();
    let first = header
        .dataset()
        .int(tags::FIRST_ROOT_RECORD)
        .and_then(|o| u64::try_from(o).ok())
        .and_then(|o| by_offset.get(&o).copied());

    let walked = match first {
        Some(first) => walk(records, &by_offset, first)?,
        None => in_order(records),
    };
    let mut entries = Vec::new();
    for (index, context) in walked {
        let record = &records[index];
        if record.int(tags::RECORD_IN_USE) == Some(0) {
            continue;
        }
        let components = record.strings(tags::REFERENCED_FILE_ID);
        if components.is_empty() {
            continue;
        }
        let mut path = root.to_path_buf();
        for component in &components {
            if !is_one_name(component) {
                return Err(DicomError::Malformed {
                    offset: record.offset(),
                    what: "a Referenced File ID that leaves the DICOMDIR's directory".into(),
                });
            }
            path.push(component);
        }
        entries.push(DicomDirEntry {
            path,
            record_type: record_type(record),
            study_uid: context.study_uid,
            series_uid: context.series_uid,
            sop_uid: record.string(tags::REFERENCED_SOP_INSTANCE_UID_IN_FILE),
            modality: context.modality,
        });
    }
    Ok(entries)
}

/// One plain name: not empty, not `.` or `..`, no separator, not absolute.
fn is_one_name(component: &str) -> bool {
    let mut parts = Path::new(component).components();
    matches!(parts.next(), Some(Component::Normal(_)))
        && parts.next().is_none()
        && !component.contains(['/', ':', '\0'])
}

/// Depth first from `first`: a record, then its lower level, then its next.
fn walk(
    records: &[Dataset],
    by_offset: &HashMap<u64, usize>,
    first: usize,
) -> Result<Vec<(usize, Context)>, DicomError> {
    let link = |record: &Dataset, tag| -> Result<Option<usize>, DicomError> {
        match record.int(tag).and_then(|o| u64::try_from(o).ok()) {
            None | Some(0) => Ok(None),
            Some(offset) => {
                by_offset
                    .get(&offset)
                    .copied()
                    .map(Some)
                    .ok_or_else(|| DicomError::Malformed {
                        offset: record.offset(),
                        what: "a directory record points at no record".into(),
                    })
            }
        }
    };
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut stack = vec![(first, Context::default())];
    while let Some((index, context)) = stack.pop() {
        if !seen.insert(index) {
            return Err(DicomError::Malformed {
                offset: records[index].offset(),
                what: "directory records that point in a cycle".into(),
            });
        }
        let record = &records[index];
        let here = context.enter(record);
        out.push((index, here.clone()));
        if let Some(next) = link(record, tags::NEXT_RECORD)? {
            stack.push((next, context));
        }
        if let Some(lower) = link(record, tags::LOWER_RECORD)? {
            stack.push((lower, here));
        }
    }
    Ok(out)
}

/// Sequence order, each record under the last patient, study and series
/// before it.
fn in_order(records: &[Dataset]) -> Vec<(usize, Context)> {
    let mut patient = Context::default();
    let mut study = Context::default();
    let mut series = Context::default();
    let mut out = Vec::new();
    for (index, record) in records.iter().enumerate() {
        let context = match record_type(record).as_str() {
            "PATIENT" => {
                patient = Context::default().enter(record);
                study = patient.clone();
                series = patient.clone();
                patient.clone()
            }
            "STUDY" => {
                study = patient.enter(record);
                series = study.clone();
                study.clone()
            }
            "SERIES" => {
                series = study.enter(record);
                series.clone()
            }
            _ => series.enter(record),
        };
        out.push((index, context));
    }
    out
}
