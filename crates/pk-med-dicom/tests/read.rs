//! The header reader, DICOMDIR and summaries against synthetic files written
//! by `synthetic`. Offline; every value is a dummy.

use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use pk_med_dicom::synthetic::{part10, Node, SyntheticStudy, TransferSyntax};
use pk_med_dicom::{
    read_dicomdir, read_file, read_header, tags, DicomError, StudySummary, Tag, Value, Vr,
};

fn scratch(name: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pk-med-dicom-{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const ALL: [TransferSyntax; 4] = [
    TransferSyntax::ExplicitLittle,
    TransferSyntax::ImplicitLittle,
    TransferSyntax::ExplicitBig,
    TransferSyntax::Deflated,
];

#[test]
fn every_transfer_syntax_reads_the_same_header() {
    for syntax in ALL {
        let dir = Scratch(scratch("syntax"));
        let study = SyntheticStudy::new("CT").transfer_syntax(syntax);
        let files = study.write_to(&dir.0).unwrap();
        let h = read_file(&files[0]).unwrap();
        let who = format!("{syntax:?}");
        assert_eq!(h.transfer_syntax(), Some(syntax.uid()), "{who}");
        assert_eq!(h.study_uid(), Some(study.study_uid().as_str()), "{who}");
        assert_eq!(h.series_uid(), Some(study.series_uid(1).as_str()), "{who}");
        assert_eq!(
            h.sop_instance_uid(),
            Some(study.instance_uid(1, 1).as_str()),
            "{who}"
        );
        assert_eq!(h.modality(), Some("CT"), "{who}");
        assert_eq!(h.study_date().as_deref(), Some("2026-03-04"), "{who}");
        assert_eq!(h.study_time(), Some("101500"), "{who}");
        assert_eq!(h.study_description(), Some("Synthetic study"), "{who}");
        assert_eq!(h.series_description(), Some("Series 1"), "{who}");
        assert_eq!(h.body_part(), Some("ABDOMEN"), "{who}");
        assert_eq!(h.institution(), Some("Example Imaging"), "{who}");
        assert_eq!(h.accession(), Some("ACC000001"), "{who}");
        assert_eq!(
            h.referring_physician().as_deref(),
            Some("Example Clinician"),
            "{who}"
        );
        assert_eq!(h.series_number(), Some(1), "{who}");
        assert_eq!(h.instance_number(), Some(1), "{who}");
        assert_eq!((h.rows(), h.columns()), (Some(2), Some(2)), "{who}");
        assert_eq!(h.number_of_frames(), None, "{who}");
        assert_eq!(
            h.patient_name().as_deref(),
            Some("Robin Testperson"),
            "{who}"
        );
        assert_eq!(h.get(tags::PATIENT_NAME), Some("Testperson^Robin"), "{who}");
        assert_eq!(h.patient_id(), Some("PID000001"), "{who}");
        assert_eq!(
            h.patient_birth_date().as_deref(),
            Some("1970-01-01"),
            "{who}"
        );
        assert!(h.has_pixel_data(), "{who}");
        // The undefined-length Referenced Study Sequence, one item.
        let referenced = h.dataset().items(tags::REFERENCED_STUDY_SEQUENCE);
        assert_eq!(referenced.len(), 1, "{who}");
        assert_eq!(
            referenced[0].string(tags::REFERENCED_SOP_INSTANCE_UID),
            Some(study.study_uid()),
            "{who}"
        );
        // The icon's encapsulated pixel data is skipped, not read.
        let icon = &h.dataset().items(tags::ICON_IMAGE_SEQUENCE)[0];
        assert_eq!(
            icon.get(tags::PIXEL_DATA).map(|e| &e.value),
            Some(&Value::Encapsulated),
            "{who}"
        );
        // Nothing after Pixel Data is read.
        assert!(h.element(Tag(0xFFFC, 0xFFFC)).is_none(), "{who}");
        assert!(h.element(tags::PIXEL_DATA).is_none(), "{who}");
    }
}

/// Counts what is read from it.
struct Counting<'a> {
    bytes: &'a [u8],
    read: usize,
}

impl Read for Counting<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = buf.len().min(self.bytes.len() - self.read);
        buf[..n].copy_from_slice(&self.bytes[self.read..self.read + n]);
        self.read += n;
        Ok(n)
    }
}

#[test]
fn reading_stops_at_the_pixel_data_tag() {
    // 1 MiB of pixels; the reader's buffer is 8 KiB.
    let pixels = vec![0x5Au8; 1024 * 1024];
    let bytes = le(&[
        Node::text(tags::MODALITY, Vr::CS, "MR"),
        Node::bytes(tags::PIXEL_DATA, Vr::OB, &pixels),
    ]);
    // The top-level Pixel Data element: (7FE0,0010) OB, explicit little endian.
    let at = bytes
        .windows(6)
        .position(|w| w == [0xE0, 0x7F, 0x10, 0x00, b'O', b'B'])
        .unwrap();
    let mut counting = Counting {
        bytes: &bytes,
        read: 0,
    };
    let h = read_header(&mut counting).unwrap();
    assert!(h.has_pixel_data());
    assert_eq!(h.modality(), Some("MR"));
    assert!(h.element(tags::PIXEL_DATA).is_none());
    assert!(
        counting.read <= at + 4 + 8 * 1024,
        "read {} of {}, Pixel Data at {at}",
        counting.read,
        bytes.len()
    );
}

#[test]
fn a_file_without_a_preamble_reads() {
    let study = SyntheticStudy::new("CR");
    let dir = Scratch(scratch("nopreamble"));
    let files = study.write_to(&dir.0).unwrap();
    let bytes = std::fs::read(&files[0]).unwrap();
    assert!(pk_med_dicom::is_dicom(&bytes[..132]));
    assert!(pk_med_dicom::is_dicom(&bytes[132..]));
    let h = read_header(&bytes[132..]).unwrap();
    assert_eq!(h.study_uid(), Some(study.study_uid().as_str()));
    assert!(!pk_med_dicom::is_dicom(b"%PDF-1.7 not a DICOM file at all"));
}

#[test]
fn not_dicom_is_exit_2_and_a_cut_file_says_where() {
    let err = read_header(&b"%PDF-1.7\n"[..]).unwrap_err();
    assert!(matches!(err, DicomError::NotDicom), "{err}");
    assert_eq!(pk_cli_core::CliError::from(err).exit_code(), 2);

    let study = SyntheticStudy::new("CT");
    let dir = Scratch(scratch("cut"));
    let files = study.write_to(&dir.0).unwrap();
    let bytes = std::fs::read(&files[0]).unwrap();
    let name = bytes
        .windows(16)
        .position(|w| w == b"Testperson^Robin")
        .unwrap();
    let err = read_header(&bytes[..name + 4]).unwrap_err();
    assert!(matches!(err, DicomError::Truncated { .. }), "{err}");
    let text = err.to_string();
    assert!(!text.contains("Test"), "no value in an error: {text}");
    assert_eq!(pk_cli_core::CliError::from(err).exit_code(), 1);
}

fn le(nodes: &[Node]) -> Vec<u8> {
    part10(
        "1.2.840.10008.5.1.4.1.1.7",
        "2.25.0.9",
        TransferSyntax::ExplicitLittle,
        nodes,
    )
}

#[test]
fn a_huge_text_value_is_refused_and_a_huge_bulk_one_skipped() {
    // (0008,1030) LO claiming 64 KiB is fine; a UT claiming 1 GiB is refused
    // before anything is allocated.
    let mut ut = vec![0x08, 0x00, 0x30, 0x10, b'U', b'T', 0, 0];
    ut.extend_from_slice(&0x4000_0000u32.to_le_bytes());
    let err = read_header(&le(&[Node::Raw(ut)])[..]).unwrap_err();
    assert!(matches!(err, DicomError::Limit { .. }), "{err}");

    // An OB of 17 MiB is skipped and reading goes on to the next element.
    let big = vec![0u8; 17 * 1024 * 1024];
    let bytes = le(&[
        Node::bytes(Tag(0x0009, 0x1001), Vr::OB, &big),
        Node::text(tags::MODALITY, Vr::CS, "OT"),
    ]);
    let h = read_header(&bytes[..]).unwrap();
    assert_eq!(
        h.element(Tag(0x0009, 0x1001)).map(|e| &e.value),
        Some(&Value::Skipped {
            length: big.len() as u64
        })
    );
    assert_eq!(h.modality(), Some("OT"));
}

#[test]
fn nesting_past_the_limit_is_refused() {
    let mut node = Node::text(tags::MODALITY, Vr::CS, "OT");
    for _ in 0..17 {
        node = Node::sequence(Tag(0x0040, 0xA730), vec![vec![node]], true);
    }
    let err = read_header(&le(&[node])[..]).unwrap_err();
    assert!(matches!(err, DicomError::Limit { .. }), "{err}");

    let mut node = Node::text(tags::MODALITY, Vr::CS, "OT");
    for _ in 0..16 {
        node = Node::sequence(Tag(0x0040, 0xA730), vec![vec![node]], false);
    }
    assert!(read_header(&le(&[node])[..]).is_ok());
}

#[test]
fn an_unknown_element_of_undefined_length_is_an_implicit_sequence() {
    let mut un = vec![
        0x09, 0x00, 0x10, 0x10, b'U', b'N', 0, 0, 0xFF, 0xFF, 0xFF, 0xFF,
    ];
    // Item, undefined length; inside, implicit VR little endian.
    un.extend_from_slice(&[0xFE, 0xFF, 0x00, 0xE0, 0xFF, 0xFF, 0xFF, 0xFF]);
    un.extend_from_slice(&[0x08, 0x00, 0x60, 0x00, 2, 0, 0, 0, b'O', b'T']);
    un.extend_from_slice(&[0xFE, 0xFF, 0x0D, 0xE0, 0, 0, 0, 0]);
    un.extend_from_slice(&[0xFE, 0xFF, 0xDD, 0xE0, 0, 0, 0, 0]);
    let bytes = le(&[
        Node::Raw(un),
        Node::text(tags::STUDY_DESCRIPTION, Vr::LO, "after"),
    ]);
    let h = read_header(&bytes[..]).unwrap();
    let items = h.dataset().items(Tag(0x0009, 0x1010));
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].string(tags::MODALITY).as_deref(), Some("OT"));
    assert_eq!(h.study_description(), Some("after"));
}

#[test]
fn debug_shows_no_value() {
    let dir = Scratch(scratch("debug"));
    let files = SyntheticStudy::new("CT").write_to(&dir.0).unwrap();
    let h = read_file(&files[0]).unwrap();
    let shown = format!("{h:?} {:?}", h.element(tags::PATIENT_NAME).unwrap());
    for value in ["Testperson", "PID000001", "1970"] {
        assert!(!shown.contains(value), "{value} in {shown}");
    }
}

#[test]
fn a_dicomdir_lists_every_file_under_its_study_and_series() {
    for offsets in [true, false] {
        let dir = Scratch(scratch("dicomdir"));
        let mut study = SyntheticStudy::new("MR").series(2).instances(3);
        if !offsets {
            study = study.dicomdir_without_offsets();
        }
        let files = study.write_to(&dir.0).unwrap();
        let dicomdir = read_file(dir.0.join("DICOMDIR")).unwrap();
        let records = dicomdir.dataset().items(tags::DIRECTORY_RECORD_SEQUENCE);
        assert_eq!(records.len(), 1 + 1 + 2 * (1 + 3));
        let first = dicomdir.dataset().int(tags::FIRST_ROOT_RECORD);
        // With offsets, the first root record's names the patient record's
        // Item tag, so the walk, not the fallback, is what is tested.
        assert_eq!(
            first,
            Some(if offsets {
                records[0].offset() as i64
            } else {
                0
            }),
            "offsets {offsets}"
        );
        let entries = read_dicomdir(dir.0.join("DICOMDIR")).unwrap();
        let paths: Vec<_> = entries.iter().map(|e| e.path.clone()).collect();
        assert_eq!(paths, files, "offsets {offsets}");
        for (i, entry) in entries.iter().enumerate() {
            let series = i / 3 + 1;
            let instance = i % 3 + 1;
            assert_eq!(entry.record_type, "IMAGE");
            assert_eq!(entry.study_uid, Some(study.study_uid()));
            assert_eq!(entry.series_uid, Some(study.series_uid(series)));
            assert_eq!(entry.sop_uid, Some(study.instance_uid(series, instance)));
            assert_eq!(entry.modality.as_deref(), Some("MR"));
            assert!(entry.path.exists());
        }
    }
}

#[test]
fn a_dicomdir_path_that_climbs_out_is_refused() {
    let record = |file_id: &str| {
        vec![
            Node::ul(tags::NEXT_RECORD, 0),
            Node::us(tags::RECORD_IN_USE, 0xFFFF),
            Node::ul(tags::LOWER_RECORD, 0),
            Node::text(tags::RECORD_TYPE, Vr::CS, "IMAGE"),
            Node::text(tags::REFERENCED_FILE_ID, Vr::CS, file_id),
        ]
    };
    for bad in ["..\\ETC\\PASSWD", "DICOM\\..\\..\\X", "/ETC", "A/B"] {
        let bytes = part10(
            "1.2.840.10008.1.3.10",
            "2.25.0.9",
            TransferSyntax::ExplicitLittle,
            &[Node::sequence(
                tags::DIRECTORY_RECORD_SEQUENCE,
                vec![record("DICOM\\IMG1"), record(bad)],
                false,
            )],
        );
        let h = read_header(&bytes[..]).unwrap();
        let err =
            pk_med_dicom::dicomdir_entries(&h, std::path::Path::new("/media/disc")).unwrap_err();
        assert!(matches!(err, DicomError::Malformed { .. }), "{bad}: {err}");
    }
}

#[test]
fn a_record_not_in_use_is_left_out() {
    let record = |in_use: u16, file: &str| {
        vec![
            Node::ul(tags::NEXT_RECORD, 0),
            Node::us(tags::RECORD_IN_USE, in_use),
            Node::ul(tags::LOWER_RECORD, 0),
            Node::text(tags::RECORD_TYPE, Vr::CS, "IMAGE"),
            Node::text(tags::REFERENCED_FILE_ID, Vr::CS, file),
        ]
    };
    let bytes = part10(
        "1.2.840.10008.1.3.10",
        "2.25.0.9",
        TransferSyntax::ExplicitLittle,
        &[Node::sequence(
            tags::DIRECTORY_RECORD_SEQUENCE,
            vec![record(0, "GONE"), record(0xFFFF, "KEPT")],
            false,
        )],
    );
    let h = read_header(&bytes[..]).unwrap();
    let entries = pk_med_dicom::dicomdir_entries(&h, std::path::Path::new("/disc")).unwrap();
    let paths: Vec<_> = entries.iter().map(|e| e.path.clone()).collect();
    assert_eq!(paths, [PathBuf::from("/disc/KEPT")]);
}

#[test]
fn records_that_point_in_a_cycle_are_refused() {
    // Two records whose Next fields point at each other. The first starts
    // 12 bytes into the sequence's value; both have the same size.
    let record = |next: u32| {
        vec![
            Node::ul(tags::NEXT_RECORD, next),
            Node::us(tags::RECORD_IN_USE, 0xFFFF),
            Node::ul(tags::LOWER_RECORD, 0),
            Node::text(tags::RECORD_TYPE, Vr::CS, "PATIENT"),
        ]
    };
    let build = |first: u32, second: u32| {
        part10(
            "1.2.840.10008.1.3.10",
            "2.25.0.9",
            TransferSyntax::ExplicitLittle,
            &[
                Node::ul(tags::FIRST_ROOT_RECORD, first),
                Node::sequence(
                    tags::DIRECTORY_RECORD_SEQUENCE,
                    vec![record(second), record(first)],
                    false,
                ),
            ],
        )
    };
    // Lay it out once to learn the offsets, then point them at each other.
    let h = read_header(&build(0, 0)[..]).unwrap();
    let items = h.dataset().items(tags::DIRECTORY_RECORD_SEQUENCE);
    let (a, b) = (items[0].offset() as u32, items[1].offset() as u32);
    let h = read_header(&build(a, b)[..]).unwrap();
    let err = pk_med_dicom::dicomdir_entries(&h, std::path::Path::new(".")).unwrap_err();
    assert!(err.to_string().contains("cycle"), "{err}");
}

#[test]
fn studies_and_series_are_summarized_from_headers() {
    let dir = Scratch(scratch("summary"));
    let ct = SyntheticStudy::new("CT").series(2).instances(2).seed(1);
    let mr = SyntheticStudy::new("MR").instances(3).seed(2);
    let mut files = ct.write_to(dir.0.join("a")).unwrap();
    files.extend(mr.write_to(dir.0.join("b")).unwrap());
    // The same instance twice counts once.
    files.push(files[0].clone());
    let headers: Vec<_> = files.iter().map(|f| read_file(f).unwrap()).collect();
    let studies = StudySummary::from_headers(&headers);
    assert_eq!(studies.len(), 2);
    let summary: Vec<_> = studies
        .iter()
        .map(|s| {
            (
                s.study_uid.clone(),
                s.modalities.clone(),
                s.series.iter().map(|x| x.instances).collect::<Vec<_>>(),
                s.instances,
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            (ct.study_uid(), vec!["CT".to_string()], vec![2, 2], 4),
            (mr.study_uid(), vec!["MR".to_string()], vec![3], 3),
        ]
    );
    assert_eq!(studies[0].date.as_deref(), Some("2026-03-04"));
    assert_eq!(studies[0].series[1].uid, ct.series_uid(2));
    assert_eq!(studies[0].series[0].body_part.as_deref(), Some("ABDOMEN"));
    let json = serde_json::to_string(&studies).unwrap();
    assert!(
        !json.contains("Testperson") && !json.contains("PID"),
        "{json}"
    );
}

#[test]
fn a_synthetic_study_is_written_as_a_disc_holds_it() {
    let dir = Scratch(scratch("layout"));
    let study = SyntheticStudy::new("US").seed(7);
    let files = study.write_to(&dir.0).unwrap();
    let names: Vec<_> = files
        .iter()
        .map(|f| f.strip_prefix(&dir.0).unwrap().to_path_buf())
        .collect();
    assert_eq!(
        names,
        ["IMAGES/S1/I1", "IMAGES/S1/I2", "IMAGES/S1/I3"].map(PathBuf::from)
    );
    assert_eq!(study.study_uid(), "2.25.0.7");
    assert!(pk_med_dicom::is_dicom_file(&files[0]));
    let dicomdir = read_file(dir.0.join("DICOMDIR")).unwrap();
    assert_eq!(
        dicomdir.sop_class_uid(),
        Some(pk_med_dicom::MEDIA_STORAGE_DIRECTORY)
    );
    // Dates are given either way.
    let other = SyntheticStudy::new("CT")
        .study_date("2026-02-10")
        .patient_birth_date("1970-01-01")
        .body_part("CHEST")
        .institution("Example Clinic")
        .write_to(dir.0.join("other"))
        .unwrap();
    let h = read_file(&other[0]).unwrap();
    assert_eq!(h.study_date().as_deref(), Some("2026-02-10"));
    assert_eq!(h.patient_birth_date().as_deref(), Some("1970-01-01"));
    assert_eq!(
        (h.body_part(), h.institution()),
        (Some("CHEST"), Some("Example Clinic"))
    );
}

#[test]
fn a_tag_is_a_tag_or_a_number() {
    let dir = Scratch(scratch("tag"));
    let files = SyntheticStudy::new("CT").write_to(&dir.0).unwrap();
    let h = read_file(&files[0]).unwrap();
    assert_eq!(h.get(0x0008_0060u32), Some("CT"));
    assert_eq!(h.get(0x0008_0060u32), h.get(tags::MODALITY));
    assert_eq!(Tag::from(0x0020_000Du32), tags::STUDY_INSTANCE_UID);
    assert_eq!(u32::from(tags::PIXEL_DATA), 0x7FE0_0010);
    // A number is not text; `element` reads it.
    assert_eq!(h.get(tags::ROWS), None);
    assert_eq!(h.element(tags::ROWS).and_then(|e| e.int()), Some(2));
}

#[test]
fn frames_latin1_and_an_empty_value() {
    let bytes = le(&[
        Node::bytes(tags::INSTITUTION_NAME, Vr::LO, b"Caf\xE9 "),
        Node::text(tags::STUDY_DESCRIPTION, Vr::LO, ""),
        Node::text(tags::NUMBER_OF_FRAMES, Vr::IS, "12"),
    ]);
    let h = read_header(&bytes[..]).unwrap();
    assert_eq!(h.institution(), Some("Caf\u{e9}"));
    assert_eq!(h.study_description(), None);
    assert_eq!(h.number_of_frames(), Some(12));
    assert_eq!(pk_med_dicom::Header::default().study_uid(), None);
}

#[test]
fn names_read_as_written_and_dates_only_when_real() {
    use pk_med_dicom::{iso_date, person_name};
    assert_eq!(person_name("Testperson^Robin"), "Robin Testperson");
    assert_eq!(
        person_name("Testperson^Robin^Q^Dr^Jr"),
        "Dr Robin Q Testperson Jr"
    );
    assert_eq!(person_name("Testperson^Robin=X^Y"), "Robin Testperson");
    assert_eq!(person_name("Testperson"), "Testperson");
    // The old dotted form; files write `YYYYMMDD`, read above.
    assert_eq!(iso_date("2024.02.29").as_deref(), Some("2024-02-29"));
    assert_eq!(iso_date(" 2026.03.04 ").as_deref(), Some("2026-03-04"));
    for bad in [
        "2023.02.29",
        "2026.04.31",
        "2026.13.01",
        "2026.03.0",
        "2026.03.04-2026.03.05",
        "",
    ] {
        assert_eq!(iso_date(bad), None, "{bad}");
    }
}

#[test]
fn a_file_that_does_not_open_is_not_dicom() {
    let dir = Scratch(scratch("notdicom"));
    std::fs::create_dir_all(&dir.0).unwrap();
    let pdf = dir.0.join("scan.pdf");
    std::fs::write(&pdf, b"%PDF-1.7 not a DICOM file").unwrap();
    assert!(!pk_med_dicom::is_dicom_file(&pdf));
    assert!(!pk_med_dicom::is_dicom_file(dir.0.join("missing")));
}
