//! What a header holds: data sets of elements, read without their pixels.

use std::collections::BTreeMap;
use std::fmt;

use crate::tag::{tags, Tag, Vr};

/// An element's value.
#[derive(Clone, PartialEq, Eq)]
pub enum Value {
    /// The value's bytes as stored (text padded to an even length, numbers
    /// in the file's byte order).
    Bytes(Vec<u8>),
    /// A sequence's items.
    Sequence(Vec<Dataset>),
    /// Encapsulated fragments (a compressed icon), skipped.
    Encapsulated,
    /// A bulk value over [`crate::MAX_VALUE_BYTES`], skipped.
    Skipped { length: u64 },
}

/// Its size and shape, never its bytes: they may be a patient's name.
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bytes(bytes) => write!(f, "Bytes({} bytes)", bytes.len()),
            Value::Sequence(items) => write!(f, "Sequence({} items)", items.len()),
            Value::Encapsulated => f.write_str("Encapsulated"),
            Value::Skipped { length } => write!(f, "Skipped({length} bytes)"),
        }
    }
}

/// One data element. `Debug` shows its tag, VR and size, never its value.
#[derive(Clone, PartialEq, Eq)]
pub struct Element {
    pub tag: Tag,
    pub vr: Vr,
    pub value: Value,
    pub(crate) big_endian: bool,
}

impl Element {
    pub fn bytes(&self) -> Option<&[u8]> {
        match &self.value {
            Value::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// The whole value as text: UTF-8 where it is valid, else Latin-1 (ISO
    /// IR 100, the common single-byte set), without the padding.
    pub fn text(&self) -> Option<String> {
        let bytes = self.bytes()?;
        let text = match std::str::from_utf8(bytes) {
            Ok(text) => text.to_string(),
            Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
        };
        Some(trim(&text).to_string())
    }

    /// Each of a multi-valued text value's values (split on `\`), trimmed.
    pub fn strings(&self) -> Vec<String> {
        self.text()
            .map(|t| t.split('\\').map(|v| trim(v).to_string()).collect())
            .unwrap_or_default()
    }

    /// The first value, when it is not empty.
    pub fn string(&self) -> Option<String> {
        self.strings().into_iter().next().filter(|s| !s.is_empty())
    }

    /// The first value as a whole number: binary for `US`, `UL`, `SS` and
    /// `SL`, text for `IS` and the rest.
    pub fn int(&self) -> Option<i64> {
        let bytes = self.bytes()?;
        let be = self.big_endian;
        let take = |n: usize| -> Option<Vec<u8>> {
            let mut b = bytes.get(..n)?.to_vec();
            if !be {
                b.reverse();
            }
            Some(b)
        };
        let unsigned = |b: Vec<u8>| b.iter().fold(0u64, |n, &x| (n << 8) | u64::from(x));
        match &self.vr.0 {
            b"US" => take(2).map(|b| unsigned(b) as i64),
            b"UL" => take(4).map(|b| unsigned(b) as i64),
            b"SS" => take(2).map(|b| i64::from(unsigned(b) as u16 as i16)),
            b"SL" => take(4).map(|b| i64::from(unsigned(b) as u32 as i32)),
            _ => self.string()?.trim_start_matches('+').parse().ok(),
        }
    }

    /// A sequence's items; empty for any other value.
    pub fn items(&self) -> &[Dataset] {
        match &self.value {
            Value::Sequence(items) => items,
            _ => &[],
        }
    }
}

impl fmt::Debug for Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("Element");
        d.field("tag", &self.tag).field("vr", &self.vr);
        match &self.value {
            Value::Bytes(b) => d.field("bytes", &b.len()),
            Value::Sequence(items) => d.field("items", &items.len()),
            Value::Encapsulated => d.field("encapsulated", &true),
            Value::Skipped { length } => d.field("skipped", length),
        };
        d.finish()
    }
}

fn trim(text: &str) -> &str {
    text.trim_matches(|c: char| c == ' ' || c == '\0')
}

/// A data set: the file's, or one sequence item's. `Debug` shows how many
/// elements it holds.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Dataset {
    pub(crate) elements: BTreeMap<Tag, Element>,
    pub(crate) offset: u64,
}

impl Dataset {
    pub fn get(&self, tag: Tag) -> Option<&Element> {
        self.elements.get(&tag)
    }

    pub fn string(&self, tag: Tag) -> Option<String> {
        self.get(tag)?.string()
    }

    pub fn strings(&self, tag: Tag) -> Vec<String> {
        self.get(tag).map(Element::strings).unwrap_or_default()
    }

    pub fn int(&self, tag: Tag) -> Option<i64> {
        self.get(tag)?.int()
    }

    pub fn items(&self, tag: Tag) -> &[Dataset] {
        self.get(tag).map(Element::items).unwrap_or(&[])
    }

    /// Every element, in tag order.
    pub fn iter(&self) -> impl Iterator<Item = &Element> {
        self.elements.values()
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Where a sequence item's tag starts in the file, in bytes from the
    /// file's first byte (a DICOMDIR's record offsets count this way).
    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub(crate) fn insert(&mut self, element: Element) {
        self.elements.insert(element.tag, element);
    }
}

impl fmt::Debug for Dataset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dataset")
            .field("elements", &self.elements.len())
            .finish()
    }
}

/// A DICOM file's header: its file meta information and its data set up to
/// Pixel Data. `Debug` shows the transfer syntax, the SOP class, the
/// modality and the size, and nothing about the patient.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Header {
    meta: Dataset,
    dataset: Dataset,
    transfer_syntax: String,
    has_pixel_data: bool,
    /// Every text element of the meta group and the top-level data set,
    /// decoded and without its padding.
    text: BTreeMap<Tag, String>,
}

impl Header {
    pub(crate) fn new(
        meta: Dataset,
        dataset: Dataset,
        transfer_syntax: String,
        has_pixel_data: bool,
    ) -> Header {
        let text = meta
            .iter()
            .chain(dataset.iter())
            .filter(|e| e.vr.is_text())
            .filter_map(|e| Some((e.tag, e.text()?)))
            .collect();
        Header {
            meta,
            dataset,
            transfer_syntax,
            has_pixel_data,
            text,
        }
    }

    /// The file meta information, group 0002.
    pub fn meta(&self) -> &Dataset {
        &self.meta
    }

    /// The data set, up to Pixel Data.
    pub fn dataset(&self) -> &Dataset {
        &self.dataset
    }

    /// A text element's whole value (several values keep their `\`), UTF-8
    /// where it is valid and Latin-1 otherwise, without its padding; `None`
    /// when absent, empty, or not text. Group 0002 is read from the meta
    /// information, any other from the data set's top level. A tag is a
    /// [`Tag`] or `0xGGGGEEEE`.
    pub fn get(&self, tag: impl Into<Tag>) -> Option<&str> {
        self.text
            .get(&tag.into())
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }

    /// Any element, as [`Self::get`] finds it: sequences, numbers and bytes
    /// too.
    pub fn element(&self, tag: impl Into<Tag>) -> Option<&Element> {
        let tag = tag.into();
        if tag.0 == 0x0002 {
            self.meta.get(tag)
        } else {
            self.dataset.get(tag)
        }
    }

    /// The transfer syntax UID from the meta information.
    pub fn transfer_syntax(&self) -> Option<&str> {
        Some(self.transfer_syntax.as_str()).filter(|s| !s.is_empty())
    }

    /// Whether the data set reached Pixel Data (where reading stopped).
    pub fn has_pixel_data(&self) -> bool {
        self.has_pixel_data
    }

    /// The data set's SOP Class UID, else the meta information's.
    pub fn sop_class_uid(&self) -> Option<&str> {
        self.get(tags::SOP_CLASS_UID)
            .or_else(|| self.get(tags::MEDIA_STORAGE_SOP_CLASS_UID))
    }

    /// The data set's SOP Instance UID, else the meta information's.
    pub fn sop_instance_uid(&self) -> Option<&str> {
        self.get(tags::SOP_INSTANCE_UID)
            .or_else(|| self.get(tags::MEDIA_STORAGE_SOP_INSTANCE_UID))
    }

    /// Study Instance UID.
    pub fn study_uid(&self) -> Option<&str> {
        self.get(tags::STUDY_INSTANCE_UID)
    }

    /// Series Instance UID.
    pub fn series_uid(&self) -> Option<&str> {
        self.get(tags::SERIES_INSTANCE_UID)
    }

    /// `CT`, `MR`, `US`, `DX`, ...
    pub fn modality(&self) -> Option<&str> {
        self.get(tags::MODALITY)
    }

    /// Body Part Examined: `CHEST`, `ABDOMEN`, ...
    pub fn body_part(&self) -> Option<&str> {
        self.get(tags::BODY_PART_EXAMINED)
    }

    /// Study Date as `YYYY-MM-DD`.
    pub fn study_date(&self) -> Option<String> {
        iso_date(self.get(tags::STUDY_DATE)?)
    }

    /// Series Date as `YYYY-MM-DD`.
    pub fn series_date(&self) -> Option<String> {
        iso_date(self.get(tags::SERIES_DATE)?)
    }

    /// Study Time as written (`HHMMSS.FFFFFF`, any tail left out).
    pub fn study_time(&self) -> Option<&str> {
        self.get(tags::STUDY_TIME)
    }

    pub fn study_description(&self) -> Option<&str> {
        self.get(tags::STUDY_DESCRIPTION)
    }

    pub fn series_description(&self) -> Option<&str> {
        self.get(tags::SERIES_DESCRIPTION)
    }

    pub fn series_number(&self) -> Option<i64> {
        self.element(tags::SERIES_NUMBER)?.int()
    }

    pub fn instance_number(&self) -> Option<i64> {
        self.element(tags::INSTANCE_NUMBER)?.int()
    }

    /// Number of Frames, when the file says.
    pub fn number_of_frames(&self) -> Option<u32> {
        u32::try_from(self.element(tags::NUMBER_OF_FRAMES)?.int()?).ok()
    }

    pub fn rows(&self) -> Option<i64> {
        self.element(tags::ROWS)?.int()
    }

    pub fn columns(&self) -> Option<i64> {
        self.element(tags::COLUMNS)?.int()
    }

    /// Institution Name.
    pub fn institution(&self) -> Option<&str> {
        self.get(tags::INSTITUTION_NAME)
    }

    /// Accession Number.
    pub fn accession(&self) -> Option<&str> {
        self.get(tags::ACCESSION_NUMBER)
    }

    /// Referring Physician's Name as a person writes it ([`person_name`]).
    pub fn referring_physician(&self) -> Option<String> {
        self.get(tags::REFERRING_PHYSICIAN_NAME).map(person_name)
    }

    /// Patient's Name as a person writes it ([`person_name`]). The patient
    /// getters are for the person's own terminal: never a log, a fixture or
    /// an error.
    pub fn patient_name(&self) -> Option<String> {
        self.get(tags::PATIENT_NAME).map(person_name)
    }

    /// Patient ID, as the source wrote it. As [`Self::patient_name`].
    pub fn patient_id(&self) -> Option<&str> {
        self.get(tags::PATIENT_ID)
    }

    /// Patient's Birth Date as `YYYY-MM-DD`. As [`Self::patient_name`].
    pub fn patient_birth_date(&self) -> Option<String> {
        iso_date(self.get(tags::PATIENT_BIRTH_DATE)?)
    }

    /// Patient's Sex: `M`, `F` or `O`. As [`Self::patient_name`].
    pub fn patient_sex(&self) -> Option<&str> {
        self.get(tags::PATIENT_SEX)
    }
}

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Header")
            .field("transfer_syntax", &self.transfer_syntax)
            .field("sop_class_uid", &self.sop_class_uid())
            .field("modality", &self.modality())
            .field("elements", &self.dataset.len())
            .field("has_pixel_data", &self.has_pixel_data)
            .finish_non_exhaustive()
    }
}

/// A DICOM date (`YYYYMMDD`, or the old `YYYY.MM.DD`) as `YYYY-MM-DD`;
/// `None` for anything else, a range or a day the month does not have
/// included.
pub fn iso_date(da: &str) -> Option<String> {
    let digits: String = da.trim().chars().filter(|c| *c != '.').collect();
    if digits.len() != 8 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year: u32 = digits[..4].parse().ok()?;
    let month: u32 = digits[4..6].parse().ok()?;
    let day: u32 = digits[6..8].parse().ok()?;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    (1..=days)
        .contains(&day)
        .then(|| format!("{}-{}-{}", &digits[..4], &digits[4..6], &digits[6..8]))
}

/// A person name (PN, `Family^Given^Middle^Prefix^Suffix`) as a person
/// writes it: `Prefix Given Middle Family Suffix`, empty parts left out.
/// Only the first (alphabetic) group is read; ideographic and phonetic ones
/// after `=` are not.
pub fn person_name(pn: &str) -> String {
    let alphabetic = pn.split('=').next().unwrap_or(pn);
    let parts: Vec<&str> = alphabetic.split('^').map(str::trim).collect();
    let part = |i: usize| parts.get(i).copied().unwrap_or("");
    [part(3), part(1), part(2), part(0), part(4)]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
