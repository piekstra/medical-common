//! Synthetic DICOM for tests and dogfooding: an encoder for any data set,
//! and [`SyntheticStudy`], a whole study with its DICOMDIR. Every value is
//! a dummy: the patient is `Testperson^Robin`, `PID000001`, born
//! 1970-01-01, and every UID is under `2.25.0`, a root no equipment uses
//! (`2.25` is the UUID arc, and no UUID is 0). Nothing here reads a file of
//! anyone's.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::tag::{tags, Tag, Vr};
use crate::transfer_syntax as ts;

/// How a synthetic file encodes its data set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferSyntax {
    ExplicitLittle,
    ImplicitLittle,
    /// Retired, and still found on old discs.
    ExplicitBig,
    Deflated,
}

impl TransferSyntax {
    pub fn uid(self) -> &'static str {
        match self {
            TransferSyntax::ExplicitLittle => ts::EXPLICIT_VR_LITTLE_ENDIAN,
            TransferSyntax::ImplicitLittle => ts::IMPLICIT_VR_LITTLE_ENDIAN,
            TransferSyntax::ExplicitBig => ts::EXPLICIT_VR_BIG_ENDIAN,
            TransferSyntax::Deflated => ts::DEFLATED_EXPLICIT_VR_LITTLE_ENDIAN,
        }
    }

    fn explicit(self) -> bool {
        self != TransferSyntax::ImplicitLittle
    }

    fn big_endian(self) -> bool {
        self == TransferSyntax::ExplicitBig
    }
}

/// An element's value before encoding.
#[derive(Debug, Clone, PartialEq)]
pub enum Data {
    /// Text, padded with a space (a NUL for `UI`) to an even length.
    Text(String),
    U16(Vec<u16>),
    U32(Vec<u32>),
    /// Bytes as given, padded with a zero to an even length.
    Bytes(Vec<u8>),
}

/// One element to encode.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Element {
        tag: Tag,
        vr: Vr,
        data: Data,
    },
    /// A sequence's items, with undefined lengths when `undefined`.
    Sequence {
        tag: Tag,
        items: Vec<Vec<Node>>,
        undefined: bool,
    },
    /// Encapsulated fragments of undefined length (a compressed icon).
    Fragments {
        tag: Tag,
        fragments: Vec<Vec<u8>>,
    },
    /// Bytes written as they are, for a shape the others cannot make (an
    /// unknown element of undefined length, a broken length).
    Raw(Vec<u8>),
}

impl Node {
    pub fn text(tag: Tag, vr: Vr, text: &str) -> Node {
        Node::Element {
            tag,
            vr,
            data: Data::Text(text.to_string()),
        }
    }

    pub fn uid(tag: Tag, uid: &str) -> Node {
        Node::text(tag, Vr::UI, uid)
    }

    pub fn us(tag: Tag, value: u16) -> Node {
        Node::Element {
            tag,
            vr: Vr::US,
            data: Data::U16(vec![value]),
        }
    }

    pub fn ul(tag: Tag, value: u32) -> Node {
        Node::Element {
            tag,
            vr: Vr::UL,
            data: Data::U32(vec![value]),
        }
    }

    pub fn bytes(tag: Tag, vr: Vr, bytes: &[u8]) -> Node {
        Node::Element {
            tag,
            vr,
            data: Data::Bytes(bytes.to_vec()),
        }
    }

    pub fn sequence(tag: Tag, items: Vec<Vec<Node>>, undefined: bool) -> Node {
        Node::Sequence {
            tag,
            items,
            undefined,
        }
    }
}

struct Encoder {
    explicit: bool,
    big_endian: bool,
}

impl Encoder {
    fn u16(&self, out: &mut Vec<u8>, v: u16) {
        out.extend_from_slice(&if self.big_endian {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        });
    }

    fn u32(&self, out: &mut Vec<u8>, v: u32) {
        out.extend_from_slice(&if self.big_endian {
            v.to_be_bytes()
        } else {
            v.to_le_bytes()
        });
    }

    fn tag(&self, out: &mut Vec<u8>, tag: Tag) {
        self.u16(out, tag.0);
        self.u16(out, tag.1);
    }

    fn head(&self, out: &mut Vec<u8>, tag: Tag, vr: Vr, length: u32) {
        self.tag(out, tag);
        if !self.explicit {
            self.u32(out, length);
        } else if vr.has_long_length() {
            out.extend_from_slice(&vr.0);
            out.extend_from_slice(&[0, 0]);
            self.u32(out, length);
        } else {
            out.extend_from_slice(&vr.0);
            self.u16(out, length as u16);
        }
    }

    fn value(&self, vr: Vr, data: &Data) -> Vec<u8> {
        let mut bytes = Vec::new();
        let pad = match data {
            Data::Text(text) => {
                bytes.extend_from_slice(text.as_bytes());
                if vr == Vr::UI {
                    0
                } else {
                    b' '
                }
            }
            Data::U16(values) => {
                values.iter().for_each(|&v| self.u16(&mut bytes, v));
                0
            }
            Data::U32(values) => {
                values.iter().for_each(|&v| self.u32(&mut bytes, v));
                0
            }
            Data::Bytes(b) => {
                bytes.extend_from_slice(b);
                0
            }
        };
        if bytes.len() % 2 == 1 {
            bytes.push(pad);
        }
        bytes
    }

    fn node(&self, out: &mut Vec<u8>, node: &Node) {
        match node {
            Node::Element { tag, vr, data } => {
                let value = self.value(*vr, data);
                self.head(out, *tag, *vr, value.len() as u32);
                out.extend_from_slice(&value);
            }
            Node::Sequence {
                tag,
                items,
                undefined,
            } => {
                let mut body = Vec::new();
                for item in items {
                    let content = self.nodes(item);
                    self.tag(&mut body, tags::ITEM);
                    if *undefined {
                        self.u32(&mut body, u32::MAX);
                        body.extend_from_slice(&content);
                        self.tag(&mut body, tags::ITEM_DELIMITATION);
                        self.u32(&mut body, 0);
                    } else {
                        self.u32(&mut body, content.len() as u32);
                        body.extend_from_slice(&content);
                    }
                }
                if *undefined {
                    self.head(out, *tag, Vr::SQ, u32::MAX);
                    out.extend_from_slice(&body);
                    self.tag(out, tags::SEQUENCE_DELIMITATION);
                    self.u32(out, 0);
                } else {
                    self.head(out, *tag, Vr::SQ, body.len() as u32);
                    out.extend_from_slice(&body);
                }
            }
            Node::Fragments { tag, fragments } => {
                self.head(out, *tag, Vr::OB, u32::MAX);
                for fragment in fragments {
                    self.tag(out, tags::ITEM);
                    self.u32(out, fragment.len() as u32);
                    out.extend_from_slice(fragment);
                }
                self.tag(out, tags::SEQUENCE_DELIMITATION);
                self.u32(out, 0);
            }
            Node::Raw(bytes) => out.extend_from_slice(bytes),
        }
    }

    fn nodes(&self, nodes: &[Node]) -> Vec<u8> {
        let mut out = Vec::new();
        for node in nodes {
            self.node(&mut out, node);
        }
        out
    }
}

/// A data set encoded in `syntax` (deflated: explicit VR little endian, not
/// yet compressed).
pub fn encode(nodes: &[Node], syntax: TransferSyntax) -> Vec<u8> {
    Encoder {
        explicit: syntax.explicit(),
        big_endian: syntax.big_endian(),
    }
    .nodes(nodes)
}

/// The 128-byte preamble, `DICM`, and the file meta information.
fn preamble_and_meta(sop_class: &str, sop_instance: &str, syntax: TransferSyntax) -> Vec<u8> {
    let le = TransferSyntax::ExplicitLittle;
    let meta = encode(
        &[
            Node::bytes(Tag(0x0002, 0x0001), Vr::OB, &[0x00, 0x01]),
            Node::uid(tags::MEDIA_STORAGE_SOP_CLASS_UID, sop_class),
            Node::uid(tags::MEDIA_STORAGE_SOP_INSTANCE_UID, sop_instance),
            Node::uid(tags::TRANSFER_SYNTAX_UID, syntax.uid()),
            Node::uid(Tag(0x0002, 0x0012), "2.25.0"),
            Node::text(Tag(0x0002, 0x0013), Vr::SH, "PK_MED_DICOM"),
        ],
        le,
    );
    let mut out = vec![0u8; 128];
    out.extend_from_slice(b"DICM");
    out.extend_from_slice(&encode(
        &[Node::ul(tags::FILE_META_GROUP_LENGTH, meta.len() as u32)],
        le,
    ));
    out.extend_from_slice(&meta);
    out
}

/// A whole Part 10 file: preamble, meta information, and `nodes` in
/// `syntax`, deflated for [`TransferSyntax::Deflated`].
pub fn part10(
    sop_class: &str,
    sop_instance: &str,
    syntax: TransferSyntax,
    nodes: &[Node],
) -> Vec<u8> {
    let mut out = preamble_and_meta(sop_class, sop_instance, syntax);
    let dataset = encode(nodes, syntax);
    if syntax == TransferSyntax::Deflated {
        let mut deflate =
            flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
        deflate.write_all(&dataset).expect("writing to memory");
        out.extend_from_slice(&deflate.finish().expect("writing to memory"));
    } else {
        out.extend_from_slice(&dataset);
    }
    out
}

/// The SOP class of a modality's images; Secondary Capture otherwise.
pub fn sop_class_for(modality: &str) -> &'static str {
    match modality {
        "CT" => "1.2.840.10008.5.1.4.1.1.2",
        "MR" => "1.2.840.10008.5.1.4.1.1.4",
        "CR" => "1.2.840.10008.5.1.4.1.1.1",
        "DX" => "1.2.840.10008.5.1.4.1.1.1.1",
        "MG" => "1.2.840.10008.5.1.4.1.1.1.2",
        "US" => "1.2.840.10008.5.1.4.1.1.6.1",
        "NM" => "1.2.840.10008.5.1.4.1.1.20",
        "PT" => "1.2.840.10008.5.1.4.1.1.128",
        "XA" => "1.2.840.10008.5.1.4.1.1.12.1",
        _ => "1.2.840.10008.5.1.4.1.1.7",
    }
}

/// The DICOMDIR's Media Storage SOP Class.
pub use crate::MEDIA_STORAGE_DIRECTORY;

/// A synthetic study, written as a disc would hold it:
/// `DICOMDIR` and `IMAGES/S1/I1`, ...
///
/// ```
/// # use pk_med_dicom::synthetic::SyntheticStudy;
/// let dir = std::env::temp_dir().join(format!("pk-med-dicom-doc-{}", std::process::id()));
/// let files = SyntheticStudy::new("CT").series(2).write_to(&dir).unwrap();
/// assert_eq!(files.len(), 6);
/// # std::fs::remove_dir_all(&dir).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct SyntheticStudy {
    modality: String,
    series: usize,
    instances: usize,
    patient_name: String,
    patient_id: String,
    birth_date: String,
    study_date: String,
    description: String,
    body_part: String,
    institution: String,
    accession: String,
    syntax: TransferSyntax,
    dicomdir: bool,
    dicomdir_offsets: bool,
    seed: u32,
}

impl SyntheticStudy {
    /// One series of three instances, explicit VR little endian, with a
    /// DICOMDIR: a study of 2026-03-04, `ABDOMEN`, at `Example Imaging`.
    pub fn new(modality: &str) -> Self {
        SyntheticStudy {
            modality: modality.to_string(),
            series: 1,
            instances: 3,
            patient_name: "Testperson^Robin".into(),
            patient_id: "PID000001".into(),
            birth_date: "1970-01-01".into(),
            study_date: "2026-03-04".into(),
            description: "Synthetic study".into(),
            body_part: "ABDOMEN".into(),
            institution: "Example Imaging".into(),
            accession: "ACC000001".into(),
            syntax: TransferSyntax::ExplicitLittle,
            dicomdir: true,
            dicomdir_offsets: true,
            seed: 1,
        }
    }

    pub fn series(mut self, n: usize) -> Self {
        self.series = n.max(1);
        self
    }

    /// Instances in each series.
    pub fn instances(mut self, n: usize) -> Self {
        self.instances = n.max(1);
        self
    }

    /// A dummy name only (`Family^Given`).
    pub fn patient_name(mut self, name: &str) -> Self {
        self.patient_name = name.to_string();
        self
    }

    pub fn patient_id(mut self, id: &str) -> Self {
        self.patient_id = id.to_string();
        self
    }

    /// A dummy date only, `YYYYMMDD` or `YYYY-MM-DD`.
    pub fn patient_birth_date(mut self, date: &str) -> Self {
        self.birth_date = date.to_string();
        self
    }

    /// `YYYYMMDD` or `YYYY-MM-DD`.
    pub fn study_date(mut self, date: &str) -> Self {
        self.study_date = date.to_string();
        self
    }

    pub fn description(mut self, text: &str) -> Self {
        self.description = text.to_string();
        self
    }

    /// Body Part Examined: `CHEST`, `ABDOMEN`, ...
    pub fn body_part(mut self, part: &str) -> Self {
        self.body_part = part.to_string();
        self
    }

    pub fn institution(mut self, name: &str) -> Self {
        self.institution = name.to_string();
        self
    }

    pub fn accession(mut self, number: &str) -> Self {
        self.accession = number.to_string();
        self
    }

    pub fn transfer_syntax(mut self, syntax: TransferSyntax) -> Self {
        self.syntax = syntax;
        self
    }

    /// Whether to write the DICOMDIR (default yes).
    pub fn dicomdir(mut self, write: bool) -> Self {
        self.dicomdir = write;
        self
    }

    /// Write the DICOMDIR's record offsets as zero, as some writers do, so
    /// a reader must fall back to the records' order.
    pub fn dicomdir_without_offsets(mut self) -> Self {
        self.dicomdir_offsets = false;
        self
    }

    /// Tells two studies' UIDs apart.
    pub fn seed(mut self, seed: u32) -> Self {
        self.seed = seed;
        self
    }

    /// `2.25.0.<seed>`.
    pub fn study_uid(&self) -> String {
        format!("2.25.0.{}", self.seed)
    }

    /// `<study>.<series>`, from 1.
    pub fn series_uid(&self, series: usize) -> String {
        format!("{}.{series}", self.study_uid())
    }

    /// `<series>.<instance>`, from 1.
    pub fn instance_uid(&self, series: usize, instance: usize) -> String {
        format!("{}.{instance}", self.series_uid(series))
    }

    /// The file a series' instance is written to, relative to the study's
    /// directory, as DICOMDIR components: `IMAGES`, `S<series>`,
    /// `I<instance>`.
    pub fn file_id(series: usize, instance: usize) -> [String; 3] {
        [
            "IMAGES".into(),
            format!("S{series}"),
            format!("I{instance}"),
        ]
    }

    fn da(date: &str) -> String {
        date.replace('-', "")
    }

    fn instance(&self, series: usize, instance: usize) -> Vec<Node> {
        let sop_class = sop_class_for(&self.modality);
        let study_uid = self.study_uid();
        vec![
            Node::text(tags::SPECIFIC_CHARACTER_SET, Vr::CS, "ISO_IR 192"),
            Node::uid(tags::SOP_CLASS_UID, sop_class),
            Node::uid(tags::SOP_INSTANCE_UID, &self.instance_uid(series, instance)),
            Node::text(tags::STUDY_DATE, Vr::DA, &Self::da(&self.study_date)),
            Node::text(tags::SERIES_DATE, Vr::DA, &Self::da(&self.study_date)),
            Node::text(tags::STUDY_TIME, Vr::TM, "101500"),
            Node::text(tags::ACCESSION_NUMBER, Vr::SH, &self.accession),
            Node::text(tags::MODALITY, Vr::CS, &self.modality),
            Node::text(tags::INSTITUTION_NAME, Vr::LO, &self.institution),
            Node::text(tags::REFERRING_PHYSICIAN_NAME, Vr::PN, "Clinician^Example"),
            Node::text(tags::STUDY_DESCRIPTION, Vr::LO, &self.description),
            Node::text(
                tags::SERIES_DESCRIPTION,
                Vr::LO,
                &format!("Series {series}"),
            ),
            Node::sequence(
                tags::REFERENCED_STUDY_SEQUENCE,
                vec![vec![
                    Node::uid(tags::REFERENCED_SOP_CLASS_UID, "1.2.840.10008.3.1.2.3.1"),
                    Node::uid(tags::REFERENCED_SOP_INSTANCE_UID, &study_uid),
                ]],
                true,
            ),
            Node::text(tags::PATIENT_NAME, Vr::PN, &self.patient_name),
            Node::text(tags::PATIENT_ID, Vr::LO, &self.patient_id),
            Node::text(
                tags::PATIENT_BIRTH_DATE,
                Vr::DA,
                &Self::da(&self.birth_date),
            ),
            Node::text(tags::PATIENT_SEX, Vr::CS, "O"),
            Node::text(tags::BODY_PART_EXAMINED, Vr::CS, &self.body_part),
            Node::uid(tags::STUDY_INSTANCE_UID, &study_uid),
            Node::uid(tags::SERIES_INSTANCE_UID, &self.series_uid(series)),
            Node::text(tags::STUDY_ID, Vr::SH, "1"),
            Node::text(tags::SERIES_NUMBER, Vr::IS, &series.to_string()),
            Node::text(tags::INSTANCE_NUMBER, Vr::IS, &instance.to_string()),
            Node::us(tags::SAMPLES_PER_PIXEL, 1),
            Node::text(tags::PHOTOMETRIC_INTERPRETATION, Vr::CS, "MONOCHROME2"),
            Node::us(tags::ROWS, 2),
            Node::us(tags::COLUMNS, 2),
            Node::us(tags::BITS_ALLOCATED, 8),
            Node::us(tags::BITS_STORED, 8),
            Node::us(tags::HIGH_BIT, 7),
            Node::us(tags::PIXEL_REPRESENTATION, 0),
            Node::sequence(
                tags::ICON_IMAGE_SEQUENCE,
                vec![vec![
                    Node::us(tags::ROWS, 1),
                    Node::us(tags::COLUMNS, 1),
                    Node::Fragments {
                        tag: tags::PIXEL_DATA,
                        fragments: vec![Vec::new(), vec![0xFF, 0xD8, 0xFF, 0xD9]],
                    },
                ]],
                true,
            ),
            Node::bytes(tags::PIXEL_DATA, Vr::OB, &[0x10, 0x20, 0x30, 0x40]),
            // After Pixel Data: a reader that stops there never sees it.
            Node::bytes(Tag(0xFFFC, 0xFFFC), Vr::OB, &[0, 0, 0, 0]),
        ]
    }

    /// Write the study under `dir` (created when missing) and return each
    /// instance's file, series by series.
    pub fn write_to(&self, dir: impl AsRef<Path>) -> std::io::Result<Vec<PathBuf>> {
        let dir = dir.as_ref();
        let sop_class = sop_class_for(&self.modality);
        let mut files = Vec::new();
        for series in 1..=self.series {
            for instance in 1..=self.instances {
                let path = Self::file_id(series, instance)
                    .iter()
                    .fold(dir.to_path_buf(), |p, c| p.join(c));
                std::fs::create_dir_all(path.parent().expect("under dir"))?;
                std::fs::write(
                    &path,
                    part10(
                        sop_class,
                        &self.instance_uid(series, instance),
                        self.syntax,
                        &self.instance(series, instance),
                    ),
                )?;
                files.push(path);
            }
        }
        if self.dicomdir {
            std::fs::write(dir.join("DICOMDIR"), self.dicomdir_bytes())?;
        }
        Ok(files)
    }

    /// The records, depth first: patient, study, then each series and its
    /// images. Each is (record nodes without its link elements, next, lower)
    /// by index.
    fn records(&self) -> Vec<(Vec<Node>, Option<usize>, Option<usize>)> {
        let mut records = Vec::new();
        records.push((
            vec![
                Node::text(tags::RECORD_TYPE, Vr::CS, "PATIENT"),
                Node::text(tags::PATIENT_NAME, Vr::PN, &self.patient_name),
                Node::text(tags::PATIENT_ID, Vr::LO, &self.patient_id),
            ],
            None,
            Some(1),
        ));
        records.push((
            vec![
                Node::text(tags::RECORD_TYPE, Vr::CS, "STUDY"),
                Node::text(tags::STUDY_DATE, Vr::DA, &Self::da(&self.study_date)),
                Node::text(tags::STUDY_TIME, Vr::TM, "101500"),
                Node::text(tags::ACCESSION_NUMBER, Vr::SH, &self.accession),
                Node::text(tags::STUDY_DESCRIPTION, Vr::LO, &self.description),
                Node::uid(tags::STUDY_INSTANCE_UID, &self.study_uid()),
                Node::text(tags::STUDY_ID, Vr::SH, "1"),
            ],
            None,
            Some(2),
        ));
        let per_series = 1 + self.instances;
        for series in 1..=self.series {
            let at = records.len();
            let last_series = series == self.series;
            records.push((
                vec![
                    Node::text(tags::RECORD_TYPE, Vr::CS, "SERIES"),
                    Node::text(tags::MODALITY, Vr::CS, &self.modality),
                    Node::uid(tags::SERIES_INSTANCE_UID, &self.series_uid(series)),
                    Node::text(tags::SERIES_NUMBER, Vr::IS, &series.to_string()),
                ],
                (!last_series).then_some(at + per_series),
                Some(at + 1),
            ));
            for instance in 1..=self.instances {
                let at = records.len();
                records.push((
                    vec![
                        Node::text(tags::RECORD_TYPE, Vr::CS, "IMAGE"),
                        Node::text(
                            tags::REFERENCED_FILE_ID,
                            Vr::CS,
                            &Self::file_id(series, instance).join("\\"),
                        ),
                        Node::uid(
                            tags::REFERENCED_SOP_CLASS_UID_IN_FILE,
                            sop_class_for(&self.modality),
                        ),
                        Node::uid(
                            tags::REFERENCED_SOP_INSTANCE_UID_IN_FILE,
                            &self.instance_uid(series, instance),
                        ),
                        Node::uid(Tag(0x0004, 0x1512), self.syntax.uid()),
                        Node::text(tags::INSTANCE_NUMBER, Vr::IS, &instance.to_string()),
                    ],
                    (instance < self.instances).then_some(at + 1),
                    None,
                ));
            }
        }
        records
    }

    /// The DICOMDIR: explicit VR little endian always (PS3.10 §8.6), its
    /// records with defined lengths, so each one's offset is known before
    /// it is written.
    fn dicomdir_bytes(&self) -> Vec<u8> {
        let le = TransferSyntax::ExplicitLittle;
        let records = self.records();
        let item = |nodes: &[Node], next: u32, lower: u32| {
            let mut all = vec![
                Node::ul(tags::NEXT_RECORD, next),
                Node::us(tags::RECORD_IN_USE, 0xFFFF),
                Node::ul(tags::LOWER_RECORD, lower),
            ];
            all.extend_from_slice(nodes);
            all
        };
        // Sizes do not depend on the offsets' values: every one is a UL.
        let sizes: Vec<u64> = records
            .iter()
            .map(|(nodes, _, _)| 8 + encode(&item(nodes, 0, 0), le).len() as u64)
            .collect();
        let head = |first: u32, last: u32| {
            vec![
                Node::text(Tag(0x0004, 0x1130), Vr::CS, "SYNTHETIC"),
                Node::ul(tags::FIRST_ROOT_RECORD, first),
                Node::ul(Tag(0x0004, 0x1202), last),
                Node::us(Tag(0x0004, 0x1212), 0),
            ]
        };
        let start = preamble_and_meta(MEDIA_STORAGE_DIRECTORY, &format!("{}.0", self.study_uid()), le).len()
            as u64
            + encode(&head(0, 0), le).len() as u64
            // (0004,1220) SQ: tag, VR, reserved, length.
            + 12;
        let mut offsets = Vec::with_capacity(records.len());
        let mut at = start;
        for size in &sizes {
            offsets.push(at);
            at += size;
        }
        let offset = |index: Option<usize>| -> u32 {
            if !self.dicomdir_offsets {
                return 0;
            }
            index.map_or(0, |i| offsets[i] as u32)
        };
        let items: Vec<Vec<Node>> = records
            .iter()
            .map(|(nodes, next, lower)| item(nodes, offset(*next), offset(*lower)))
            .collect();
        let root = offset(Some(0));
        let mut nodes = head(root, root);
        nodes.push(Node::sequence(
            tags::DIRECTORY_RECORD_SEQUENCE,
            items,
            false,
        ));
        part10(
            MEDIA_STORAGE_DIRECTORY,
            &format!("{}.0", self.study_uid()),
            le,
            &nodes,
        )
    }
}
