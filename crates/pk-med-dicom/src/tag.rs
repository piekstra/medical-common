//! Tags, value representations, and the few VRs an implicit-VR file needs
//! from a dictionary.

use std::fmt;

/// A data element tag, `(group, element)`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tag(pub u16, pub u16);

impl Tag {
    /// `0xGGGGEEEE` as `(GGGG,EEEE)`.
    pub const fn from_u32(tag: u32) -> Tag {
        Tag((tag >> 16) as u16, tag as u16)
    }

    /// `(GGGG,EEEE)` as `0xGGGGEEEE`.
    pub const fn as_u32(self) -> u32 {
        ((self.0 as u32) << 16) | self.1 as u32
    }

    pub const fn group(self) -> u16 {
        self.0
    }

    pub const fn element(self) -> u16 {
        self.1
    }
}

/// `0xGGGGEEEE`, so a tag can be written as one number.
impl From<u32> for Tag {
    fn from(tag: u32) -> Tag {
        Tag::from_u32(tag)
    }
}

impl From<Tag> for u32 {
    fn from(tag: Tag) -> u32 {
        tag.as_u32()
    }
}

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:04X},{:04X})", self.0, self.1)
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// The tags this crate reads by name.
pub mod tags {
    use super::Tag;

    pub const FILE_META_GROUP_LENGTH: Tag = Tag(0x0002, 0x0000);
    pub const MEDIA_STORAGE_SOP_CLASS_UID: Tag = Tag(0x0002, 0x0002);
    pub const MEDIA_STORAGE_SOP_INSTANCE_UID: Tag = Tag(0x0002, 0x0003);
    pub const TRANSFER_SYNTAX_UID: Tag = Tag(0x0002, 0x0010);

    pub const FIRST_ROOT_RECORD: Tag = Tag(0x0004, 0x1200);
    pub const DIRECTORY_RECORD_SEQUENCE: Tag = Tag(0x0004, 0x1220);
    pub const NEXT_RECORD: Tag = Tag(0x0004, 0x1400);
    pub const RECORD_IN_USE: Tag = Tag(0x0004, 0x1410);
    pub const LOWER_RECORD: Tag = Tag(0x0004, 0x1420);
    pub const RECORD_TYPE: Tag = Tag(0x0004, 0x1430);
    pub const REFERENCED_FILE_ID: Tag = Tag(0x0004, 0x1500);
    pub const REFERENCED_SOP_CLASS_UID_IN_FILE: Tag = Tag(0x0004, 0x1510);
    pub const REFERENCED_SOP_INSTANCE_UID_IN_FILE: Tag = Tag(0x0004, 0x1511);

    pub const SPECIFIC_CHARACTER_SET: Tag = Tag(0x0008, 0x0005);
    pub const SOP_CLASS_UID: Tag = Tag(0x0008, 0x0016);
    pub const SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x0018);
    pub const STUDY_DATE: Tag = Tag(0x0008, 0x0020);
    pub const SERIES_DATE: Tag = Tag(0x0008, 0x0021);
    pub const STUDY_TIME: Tag = Tag(0x0008, 0x0030);
    pub const ACCESSION_NUMBER: Tag = Tag(0x0008, 0x0050);
    pub const MODALITY: Tag = Tag(0x0008, 0x0060);
    pub const INSTITUTION_NAME: Tag = Tag(0x0008, 0x0080);
    pub const REFERRING_PHYSICIAN_NAME: Tag = Tag(0x0008, 0x0090);
    pub const STUDY_DESCRIPTION: Tag = Tag(0x0008, 0x1030);
    pub const SERIES_DESCRIPTION: Tag = Tag(0x0008, 0x103E);
    pub const REFERENCED_STUDY_SEQUENCE: Tag = Tag(0x0008, 0x1110);
    pub const REFERENCED_SOP_CLASS_UID: Tag = Tag(0x0008, 0x1150);
    pub const REFERENCED_SOP_INSTANCE_UID: Tag = Tag(0x0008, 0x1155);

    pub const PATIENT_NAME: Tag = Tag(0x0010, 0x0010);
    pub const PATIENT_ID: Tag = Tag(0x0010, 0x0020);
    pub const PATIENT_BIRTH_DATE: Tag = Tag(0x0010, 0x0030);
    pub const PATIENT_SEX: Tag = Tag(0x0010, 0x0040);

    pub const BODY_PART_EXAMINED: Tag = Tag(0x0018, 0x0015);

    pub const STUDY_INSTANCE_UID: Tag = Tag(0x0020, 0x000D);
    pub const SERIES_INSTANCE_UID: Tag = Tag(0x0020, 0x000E);
    pub const STUDY_ID: Tag = Tag(0x0020, 0x0010);
    pub const SERIES_NUMBER: Tag = Tag(0x0020, 0x0011);
    pub const INSTANCE_NUMBER: Tag = Tag(0x0020, 0x0013);

    pub const SAMPLES_PER_PIXEL: Tag = Tag(0x0028, 0x0002);
    pub const PHOTOMETRIC_INTERPRETATION: Tag = Tag(0x0028, 0x0004);
    pub const NUMBER_OF_FRAMES: Tag = Tag(0x0028, 0x0008);
    pub const ROWS: Tag = Tag(0x0028, 0x0010);
    pub const COLUMNS: Tag = Tag(0x0028, 0x0011);
    pub const BITS_ALLOCATED: Tag = Tag(0x0028, 0x0100);
    pub const BITS_STORED: Tag = Tag(0x0028, 0x0101);
    pub const HIGH_BIT: Tag = Tag(0x0028, 0x0102);
    pub const PIXEL_REPRESENTATION: Tag = Tag(0x0028, 0x0103);

    pub const ICON_IMAGE_SEQUENCE: Tag = Tag(0x0088, 0x0200);

    pub const PIXEL_DATA: Tag = Tag(0x7FE0, 0x0010);

    pub const ITEM: Tag = Tag(0xFFFE, 0xE000);
    pub const ITEM_DELIMITATION: Tag = Tag(0xFFFE, 0xE00D);
    pub const SEQUENCE_DELIMITATION: Tag = Tag(0xFFFE, 0xE0DD);
}

/// A value representation: two upper-case letters (`PN`, `UI`, `SQ`).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Vr(pub [u8; 2]);

impl Vr {
    pub const AE: Vr = Vr(*b"AE");
    pub const CS: Vr = Vr(*b"CS");
    pub const DA: Vr = Vr(*b"DA");
    pub const IS: Vr = Vr(*b"IS");
    pub const LO: Vr = Vr(*b"LO");
    pub const OB: Vr = Vr(*b"OB");
    pub const OW: Vr = Vr(*b"OW");
    pub const PN: Vr = Vr(*b"PN");
    pub const SH: Vr = Vr(*b"SH");
    pub const SL: Vr = Vr(*b"SL");
    pub const SQ: Vr = Vr(*b"SQ");
    pub const SS: Vr = Vr(*b"SS");
    pub const TM: Vr = Vr(*b"TM");
    pub const UI: Vr = Vr(*b"UI");
    pub const UL: Vr = Vr(*b"UL");
    pub const UN: Vr = Vr(*b"UN");
    pub const US: Vr = Vr(*b"US");

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(&self.0).unwrap_or("??")
    }

    /// Whether an explicit-VR element of this VR has a 4-byte length after two
    /// reserved bytes (PS3.5 §7.1.2), rather than a 2-byte one.
    pub fn has_long_length(self) -> bool {
        matches!(
            &self.0,
            b"OB"
                | b"OD"
                | b"OF"
                | b"OL"
                | b"OV"
                | b"OW"
                | b"SQ"
                | b"SV"
                | b"UC"
                | b"UN"
                | b"UR"
                | b"UT"
                | b"UV"
        )
    }

    /// A character string (PS3.5 §6.2): what [`crate::Header::get`] reads.
    pub fn is_text(self) -> bool {
        matches!(
            &self.0,
            b"AE"
                | b"AS"
                | b"CS"
                | b"DA"
                | b"DS"
                | b"DT"
                | b"IS"
                | b"LO"
                | b"LT"
                | b"PN"
                | b"SH"
                | b"ST"
                | b"TM"
                | b"UC"
                | b"UI"
                | b"UR"
                | b"UT"
        )
    }

    /// Bytes rather than text or numbers: kept, or skipped when large.
    pub fn is_bulk(self) -> bool {
        matches!(
            &self.0,
            b"OB" | b"OD" | b"OF" | b"OL" | b"OV" | b"OW" | b"UN"
        )
    }

    fn is_valid(bytes: [u8; 2]) -> bool {
        bytes.iter().all(u8::is_ascii_uppercase)
    }

    pub(crate) fn read(bytes: [u8; 2]) -> Option<Vr> {
        Vr::is_valid(bytes).then_some(Vr(bytes))
    }
}

impl fmt::Debug for Vr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The VR of a tag in an implicit-VR data set, which does not say: the tags
/// this crate reads, group lengths, private creators, and `UN` for the rest
/// (read as bytes).
pub(crate) fn implicit_vr(tag: Tag) -> Vr {
    use tags::*;
    if tag.1 == 0x0000 {
        return Vr::UL;
    }
    if tag.0 % 2 == 1 && (0x0010..=0x00FF).contains(&tag.1) {
        return Vr::LO;
    }
    match tag {
        DIRECTORY_RECORD_SEQUENCE
        | REFERENCED_STUDY_SEQUENCE
        | ICON_IMAGE_SEQUENCE
        | Tag(0x0008, 0x1115)
        | Tag(0x0008, 0x1140)
        | Tag(0x0008, 0x1199)
        | Tag(0x0040, 0x0275)
        | Tag(0x0040, 0xA730) => Vr::SQ,
        FIRST_ROOT_RECORD | Tag(0x0004, 0x1202) | NEXT_RECORD | LOWER_RECORD => Vr::UL,
        RECORD_IN_USE | Tag(0x0004, 0x1212) => Vr::US,
        SAMPLES_PER_PIXEL | ROWS | COLUMNS | BITS_ALLOCATED | BITS_STORED | HIGH_BIT
        | PIXEL_REPRESENTATION => Vr::US,
        SPECIFIC_CHARACTER_SET
        | MODALITY
        | PATIENT_SEX
        | BODY_PART_EXAMINED
        | PHOTOMETRIC_INTERPRETATION
        | RECORD_TYPE
        | REFERENCED_FILE_ID
        | Tag(0x0004, 0x1130) => Vr::CS,
        SOP_CLASS_UID
        | SOP_INSTANCE_UID
        | STUDY_INSTANCE_UID
        | SERIES_INSTANCE_UID
        | REFERENCED_SOP_CLASS_UID
        | REFERENCED_SOP_INSTANCE_UID
        | REFERENCED_SOP_CLASS_UID_IN_FILE
        | REFERENCED_SOP_INSTANCE_UID_IN_FILE
        | Tag(0x0004, 0x1512) => Vr::UI,
        STUDY_DATE | SERIES_DATE | PATIENT_BIRTH_DATE => Vr::DA,
        STUDY_TIME => Vr::TM,
        ACCESSION_NUMBER | STUDY_ID => Vr::SH,
        INSTITUTION_NAME | STUDY_DESCRIPTION | SERIES_DESCRIPTION | PATIENT_ID => Vr::LO,
        PATIENT_NAME | REFERRING_PHYSICIAN_NAME => Vr::PN,
        SERIES_NUMBER | INSTANCE_NUMBER | NUMBER_OF_FRAMES => Vr::IS,
        PIXEL_DATA => Vr::OW,
        _ => Vr::UN,
    }
}
