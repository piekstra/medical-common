//! A pull end to end, offline: the mock FHIR server, the client, the stream
//! writer, the reader and provenance, as a provider CLI and the store use them.

use pk_cli_core::block_on;
use pk_cli_http::BlockingTransport;
use pk_cli_secrets::Secret;
use pk_med_fhir::test_util::{synthetic_types, Fault, MockServer, PATIENT_ID};
use pk_med_fhir::{Binary, DocumentReference, FhirClient};
use pk_med_records::stream::skip;
use pk_med_records::test_util::{synthetic_header, write_synthetic_stream};
use pk_med_records::{Item, Provenance, RecordsReader, RecordsWriter, Skip, StreamError};

/// What a provider CLI's `records export` does, in brief: each type's search,
/// each DocumentReference's Binary after it, a skip for what failed.
fn pull(mock: &MockServer, out: &mut Vec<u8>) -> pk_med_records::Trailer {
    let client = FhirClient::new(
        BlockingTransport::new().unwrap(),
        &mock.fhir_base(),
        "pk-test",
        "0.0.0",
    )
    .unwrap()
    .bearer(Secret::new(mock.access_token()))
    .sleeper(|_| {});
    let mut header = synthetic_header();
    header.organization.fhir_base = client.base().to_string();
    let mut writer = RecordsWriter::start(out, &header).unwrap();
    for resource_type in synthetic_types() {
        if resource_type == "Binary" {
            continue;
        }
        // A skip names the search, never the patient it was for.
        let search = format!("{resource_type}?patient");
        match block_on(client.search(resource_type, &[("patient", PATIENT_ID.into())])) {
            Ok(page) => {
                for resource in &page.resources {
                    writer.resource(resource).unwrap();
                    if resource.resource_type != "DocumentReference" {
                        continue;
                    }
                    let doc: DocumentReference = resource.parse_as().unwrap();
                    for content in &doc.content {
                        let Some(url) = content.attachment.as_ref().and_then(|a| a.url.as_deref())
                        else {
                            continue;
                        };
                        match block_on(client.binary(url)) {
                            Ok(binary) => {
                                writer.resource(&binary).unwrap();
                            }
                            Err(e) => writer.skip(Skip::from_error("Binary", "", &e)),
                        }
                    }
                }
                if page.truncated {
                    writer.skip(Skip::search(
                        &search,
                        skip::PAGE_CAP,
                        "pages left at the cap",
                    ));
                }
            }
            Err(e) => {
                let code = if e.exit_code() == 3 {
                    skip::FORBIDDEN
                } else {
                    skip::UNSUPPORTED
                };
                writer.skip(Skip::search(search.clone(), code, e.message().to_string()));
            }
        }
    }
    writer.finish("2020-01-03T00:01:00Z", None).unwrap().trailer
}

#[test]
fn a_pull_streams_every_type_and_the_store_reads_it_back() {
    let mock = MockServer::start();
    mock.fail(Fault::not_supported("/fhir/Procedure"));
    let mut out = Vec::new();
    let trailer = pull(&mock, &mut out);
    assert!(trailer.complete);
    assert_eq!(trailer.counts.len(), 17, "{:?}", trailer.counts);
    assert_eq!(trailer.counts["Observation"], 3);
    assert_eq!(trailer.skipped.len(), 1);
    assert_eq!(trailer.skipped[0].code, skip::UNSUPPORTED);
    assert_eq!(
        trailer.skipped[0].resource_type.as_deref(),
        Some("Procedure")
    );

    let mut reader = RecordsReader::new(out.as_slice());
    let header = reader.header().unwrap().clone();
    let mut previous = String::new();
    let mut binaries = 0;
    for item in reader {
        match item.unwrap() {
            Item::Resource(resource) => {
                if resource.resource_type == "Binary" {
                    assert_eq!(
                        previous, "DocumentReference",
                        "a Binary follows its document"
                    );
                    let binary: Binary = resource.parse_as().unwrap();
                    assert!(binary.bytes().unwrap().starts_with(b"%PDF-"));
                    binaries += 1;
                }
                if resource.id == "obs-0001" {
                    assert!(resource.raw.contains(r#""value":1.50"#), "{}", resource.raw);
                }
                let p = Provenance::from_stream(
                    &header,
                    &resource,
                    "2020-01-04T00:00:00Z",
                    "medrec 0.0.0",
                );
                assert_eq!(p.source_resource.unwrap().id, resource.id);
                previous = resource.resource_type.clone();
            }
            Item::Trailer(t) => assert_eq!(t, trailer),
        }
    }
    assert_eq!(binaries, 1);
}

#[test]
fn a_pull_cut_short_is_detected() {
    let mut out = Vec::new();
    write_synthetic_stream(&mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    let cut: String = text.lines().take(5).map(|l| format!("{l}\n")).collect();
    let mut reader = RecordsReader::new(cut.as_bytes());
    reader.header().unwrap();
    let last = reader.last().unwrap();
    assert!(matches!(last, Err(StreamError::MissingTrailer)));
}

#[test]
fn the_synthetic_stream_has_every_type() {
    let mut out = Vec::new();
    let trailer = write_synthetic_stream(&mut out).unwrap();
    assert_eq!(trailer.counts.len(), synthetic_types().len());
    let read: Vec<Item> = {
        let mut reader = RecordsReader::new(out.as_slice());
        reader.header().unwrap();
        reader.map(Result::unwrap).collect()
    };
    assert_eq!(read.len() as u64, trailer.total() + 1);
}
