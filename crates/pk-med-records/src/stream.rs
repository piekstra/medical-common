//! The records stream (`records-stream/v1`): NDJSON, a header line, one
//! verbatim FHIR resource per line, and a trailer line
//! (`records-stream-end/v1`) that counts what was written and says whether the
//! pull finished. A stream without its trailer was cut short.
//!
//! ```text
//! {"schema":"records-stream/v1","source":{…},"organization":{…},"patient":{…},"pulled_at":"…","fhir_version":"4.0.1"}
//! {"resourceType":"Patient","id":"…",…}
//! {"resourceType":"Observation","id":"…",…}
//! {"schema":"records-stream-end/v1","complete":true,"counts":{"Observation":1,"Patient":1},"finished_at":"…"}
//! ```

use std::collections::{BTreeMap, HashSet};
use std::fmt;
use std::io::{BufRead, Write};

use pk_cli_core::CliError;
use pk_med_fhir::{minify, RawResource, FHIR_VERSION};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dto::{method, Organization, PatientContext};

pub const STREAM_SCHEMA: &str = "records-stream/v1";
pub const END_SCHEMA: &str = "records-stream-end/v1";
/// The longest line a writer writes or a reader reads unless told otherwise:
/// 64 MiB, a Binary of about 48 MiB.
pub const MAX_LINE_BYTES: usize = 64 * 1024 * 1024;

/// `Skip.code` values: an open set, so a reader treats an unknown one as a
/// skip like any other.
pub mod skip {
    /// The resource names a file the source did not serve.
    pub const NO_FILE: &str = "no_file";
    /// A file failed its check (`documents/v1`'s `verify_download`).
    pub const VERIFY_FAILED: &str = "verify_failed";
    /// The source failed (a 5xx, a 429 not waited out).
    pub const UPSTREAM: &str = "upstream";
    /// The source refused access to it (a 403, a scope not granted).
    pub const FORBIDDEN: &str = "forbidden";
    pub const NOT_FOUND: &str = "not_found";
    /// Over the line limit.
    pub const TOO_LARGE: &str = "too_large";
    /// The source does not offer the search or the type.
    pub const UNSUPPORTED: &str = "unsupported";
    /// Pages were left at the page cap.
    pub const PAGE_CAP: &str = "page_cap";
}

/// The program that wrote the stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub cli: String,
    pub version: String,
}

impl Source {
    pub fn new(cli: impl Into<String>, version: impl Into<String>) -> Self {
        Source {
            cli: cli.into(),
            version: version.into(),
        }
    }
}

/// The first line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub schema: String,
    pub source: Source,
    pub organization: Organization,
    pub patient: PatientContext,
    /// `--since`, `YYYY-MM-DD`, when the pull was bounded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    /// With `since`: the resource types the pull left unbounded, because the
    /// source documents no date parameter for them. Their resources may be
    /// older than `since`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub since_unbounded: Vec<String>,
    /// When the pull started, RFC 3339.
    pub pulled_at: String,
    #[serde(default = "fhir_version")]
    pub fhir_version: String,
    /// How the resources were obtained, a `dto::method` value: absent for
    /// `fhir-api`; `portal` for resources a CLI mapped from a member or
    /// patient site, or from files downloaded there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

fn fhir_version() -> String {
    FHIR_VERSION.into()
}

impl Header {
    pub fn new(
        source: Source,
        organization: Organization,
        patient: PatientContext,
        pulled_at: impl Into<String>,
    ) -> Self {
        Header {
            schema: STREAM_SCHEMA.into(),
            source,
            organization,
            patient,
            since: None,
            since_unbounded: Vec::new(),
            pulled_at: pulled_at.into(),
            fhir_version: fhir_version(),
            method: None,
        }
    }

    /// `method`, `fhir-api` when absent.
    pub fn method(&self) -> &str {
        self.method.as_deref().unwrap_or(method::FHIR_API)
    }
}

/// Something the pull left out, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skip {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The search left out (`Observation?category=laboratory`), with no
    /// patient id or token in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    /// For a person: what happened, with no record content in it.
    pub reason: String,
    /// One of [`skip`]'s values.
    pub code: String,
}

impl Skip {
    /// One resource left out.
    pub fn resource(
        resource_type: impl Into<String>,
        id: impl Into<String>,
        code: &str,
        reason: impl Into<String>,
    ) -> Self {
        let id = id.into();
        Skip {
            resource_type: Some(resource_type.into()),
            id: (!id.is_empty()).then_some(id),
            search: None,
            reason: reason.into(),
            code: code.into(),
        }
    }

    /// A search left out, or left unfinished.
    pub fn search(search: impl Into<String>, code: &str, reason: impl Into<String>) -> Self {
        let search = search.into();
        Skip {
            resource_type: search
                .split('?')
                .next()
                .filter(|t| !t.is_empty())
                .map(str::to_string),
            id: None,
            search: Some(search),
            reason: reason.into(),
            code: code.into(),
        }
    }

    /// The skip for a failed fetch, its code from the error's exit code.
    pub fn from_error(resource_type: &str, id: &str, err: &CliError) -> Self {
        let code = match err.exit_code() {
            3 => skip::FORBIDDEN,
            4 => skip::NOT_FOUND,
            _ => skip::UPSTREAM,
        };
        Skip::resource(resource_type, id, code, err.message().to_string())
    }
}

/// Why a stream is incomplete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrailerError {
    /// The error's code: `auth`, `not_found`, `upstream`, ...
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

impl From<&CliError> for TrailerError {
    fn from(err: &CliError) -> Self {
        TrailerError {
            code: err.code().to_string(),
            message: err.message().to_string(),
            exit_code: Some(err.exit_code()),
        }
    }
}

/// The last line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trailer {
    pub schema: String,
    /// `false` when the pull stopped on an error after the header: what was
    /// written is real but not all of it.
    pub complete: bool,
    /// Resource lines written, by type.
    #[serde(default)]
    pub counts: BTreeMap<String, u64>,
    #[serde(default)]
    pub skipped: Vec<Skip>,
    /// RFC 3339.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    /// Why `complete` is `false`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<TrailerError>,
}

impl Trailer {
    /// Resource lines in all.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }
}

fn write_error(e: std::io::Error) -> CliError {
    CliError::Other(format!("writing the records stream: {e}"))
}

/// Writes a stream: the header on [`RecordsWriter::start`], each resource
/// once by type and id, and the trailer on [`RecordsWriter::finish`].
///
/// Nothing goes to the output before `start`, so a failure before the first
/// request leaves it empty and the CLI reports the error as usual. After
/// `start`, a failure is `finish(…, Some(&err))`, which writes a trailer with
/// `complete: false`, and the CLI exits with the error's code.
pub struct RecordsWriter<W: Write> {
    out: W,
    max_line_bytes: usize,
    counts: BTreeMap<String, u64>,
    skipped: Vec<Skip>,
    seen: HashSet<(String, String)>,
    bytes: u64,
}

impl<W: Write> RecordsWriter<W> {
    /// Write the header line.
    pub fn start(mut out: W, header: &Header) -> Result<Self, CliError> {
        let line = serde_json::to_string(header)
            .map_err(|e| CliError::Other(format!("serializing the stream header: {e}")))?;
        writeln!(out, "{line}").map_err(write_error)?;
        Ok(RecordsWriter {
            out,
            max_line_bytes: MAX_LINE_BYTES,
            counts: BTreeMap::new(),
            skipped: Vec::new(),
            seen: HashSet::new(),
            bytes: line.len() as u64 + 1,
        })
    }

    /// A lower line limit than [`MAX_LINE_BYTES`].
    pub fn max_line_bytes(mut self, bytes: usize) -> Self {
        self.max_line_bytes = bytes;
        self
    }

    /// Write one resource, unless one of its type and id was written already
    /// or it is over the line limit (then it is skipped as `too_large`).
    /// Whether it was written.
    pub fn resource(&mut self, resource: &RawResource) -> Result<bool, CliError> {
        let key = (resource.resource_type.clone(), resource.id.clone());
        if !resource.id.is_empty() && self.seen.contains(&key) {
            return Ok(false);
        }
        let minified;
        let line = if resource.raw.contains('\n') {
            minified = minify(&resource.raw);
            &minified
        } else {
            &resource.raw
        };
        if line.len() > self.max_line_bytes {
            self.skip(Skip::resource(
                &resource.resource_type,
                &resource.id,
                skip::TOO_LARGE,
                format!(
                    "{} bytes, over the {}-byte line limit",
                    line.len(),
                    self.max_line_bytes
                ),
            ));
            return Ok(false);
        }
        self.out.write_all(line.as_bytes()).map_err(write_error)?;
        self.out.write_all(b"\n").map_err(write_error)?;
        self.bytes += line.len() as u64 + 1;
        if !resource.id.is_empty() {
            self.seen.insert(key);
        }
        *self
            .counts
            .entry(resource.resource_type.clone())
            .or_insert(0) += 1;
        Ok(true)
    }

    /// Whether a resource of this type and id was written.
    pub fn written(&self, resource_type: &str, id: &str) -> bool {
        self.seen
            .contains(&(resource_type.to_string(), id.to_string()))
    }

    /// Record a skip, once.
    pub fn skip(&mut self, skip: Skip) {
        if !self.skipped.contains(&skip) {
            self.skipped.push(skip);
        }
    }

    pub fn counts(&self) -> &BTreeMap<String, u64> {
        &self.counts
    }

    /// Bytes written so far.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Write the trailer and flush. `error` makes the stream incomplete.
    pub fn finish(
        mut self,
        finished_at: impl Into<String>,
        error: Option<&CliError>,
    ) -> Result<Trailer, CliError> {
        let trailer = Trailer {
            schema: END_SCHEMA.into(),
            complete: error.is_none(),
            counts: std::mem::take(&mut self.counts),
            skipped: std::mem::take(&mut self.skipped),
            finished_at: Some(finished_at.into()),
            error: error.map(TrailerError::from),
        };
        let line = serde_json::to_string(&trailer)
            .map_err(|e| CliError::Other(format!("serializing the stream trailer: {e}")))?;
        writeln!(self.out, "{line}").map_err(write_error)?;
        self.out.flush().map_err(write_error)?;
        Ok(trailer)
    }
}

/// One line after the header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Resource(RawResource),
    Trailer(Trailer),
}

/// Why a stream did not read. Every one is [`CliError::Upstream`] (exit 5):
/// the producer wrote it.
#[derive(Debug)]
pub enum StreamError {
    /// The first line is not a `records-stream/v1` header, or there is none.
    MissingHeader,
    /// The header names this schema but does not read.
    BadHeader(String),
    BadJson {
        line: u64,
        error: String,
    },
    LineTooLong {
        line: u64,
        max: usize,
    },
    /// A line with neither a `resourceType` nor the trailer's schema.
    NotAResource {
        line: u64,
    },
    /// A line after the trailer.
    AfterTrailer {
        line: u64,
    },
    /// The input ended with no trailer: the stream was cut short.
    MissingTrailer,
    /// The trailer's count for a type is not the number of lines read.
    CountMismatch {
        resource_type: String,
        trailer: u64,
        read: u64,
    },
    /// The trailer says `complete: false`. Every resource before it was
    /// yielded.
    Incomplete {
        trailer: Box<Trailer>,
    },
    Io(std::io::Error),
}

impl fmt::Display for StreamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamError::MissingHeader => {
                write!(f, "the input does not start with a {STREAM_SCHEMA} header")
            }
            StreamError::BadHeader(e) => write!(f, "the {STREAM_SCHEMA} header does not read: {e}"),
            StreamError::BadJson { line, error } => write!(f, "line {line} is not JSON: {error}"),
            StreamError::LineTooLong { line, max } => {
                write!(f, "line {line} is over the {max}-byte line limit")
            }
            StreamError::NotAResource { line } => {
                write!(f, "line {line} is neither a FHIR resource nor the trailer")
            }
            StreamError::AfterTrailer { line } => write!(f, "line {line} follows the trailer"),
            StreamError::MissingTrailer => write!(
                f,
                "the stream ended with no {END_SCHEMA} trailer: it was cut short"
            ),
            StreamError::CountMismatch {
                resource_type,
                trailer,
                read,
            } => write!(
                f,
                "the trailer counts {trailer} {resource_type}, but {read} were read"
            ),
            StreamError::Incomplete { trailer } => {
                let why = trailer
                    .error
                    .as_ref()
                    .map(|e| format!(": {} ({})", e.message, e.code))
                    .unwrap_or_default();
                write!(f, "the producer did not finish the pull{why}")
            }
            StreamError::Io(e) => write!(f, "reading the records stream: {e}"),
        }
    }
}

impl std::error::Error for StreamError {}

impl From<StreamError> for CliError {
    fn from(e: StreamError) -> Self {
        CliError::Upstream(e.to_string())
    }
}

/// Reads a stream: [`RecordsReader::header`], then each [`Item`], the trailer
/// last, checked against the lines read.
pub struct RecordsReader<R: BufRead> {
    input: R,
    max_line_bytes: usize,
    line: u64,
    header: Option<Header>,
    counts: BTreeMap<String, u64>,
    trailer_seen: bool,
    done: bool,
}

impl<R: BufRead> RecordsReader<R> {
    pub fn new(input: R) -> Self {
        RecordsReader {
            input,
            max_line_bytes: MAX_LINE_BYTES,
            line: 0,
            header: None,
            counts: BTreeMap::new(),
            trailer_seen: false,
            done: false,
        }
    }

    /// A lower (or higher) line limit than [`MAX_LINE_BYTES`].
    pub fn max_line_bytes(mut self, bytes: usize) -> Self {
        self.max_line_bytes = bytes;
        self
    }

    /// The header, read on first call.
    pub fn header(&mut self) -> Result<&Header, StreamError> {
        if self.header.is_none() {
            let Some(line) = self.next_line()? else {
                return Err(StreamError::MissingHeader);
            };
            let value: Value =
                serde_json::from_slice(&line).map_err(|_| StreamError::MissingHeader)?;
            if value.get("schema").and_then(Value::as_str) != Some(STREAM_SCHEMA) {
                return Err(StreamError::MissingHeader);
            }
            let header: Header =
                serde_json::from_value(value).map_err(|e| StreamError::BadHeader(e.to_string()))?;
            self.header = Some(header);
        }
        Ok(self.header.as_ref().expect("read above"))
    }

    /// Resource lines read so far, by type.
    pub fn counts(&self) -> &BTreeMap<String, u64> {
        &self.counts
    }

    /// The next non-empty line, without its line ending, or `None` at the end.
    fn next_line(&mut self) -> Result<Option<Vec<u8>>, StreamError> {
        loop {
            let mut line = Vec::new();
            let mut too_long = false;
            loop {
                let buf = self.input.fill_buf().map_err(StreamError::Io)?;
                if buf.is_empty() {
                    break;
                }
                let (chunk, newline) = match buf.iter().position(|&b| b == b'\n') {
                    Some(i) => (&buf[..i], Some(i)),
                    None => (buf, None),
                };
                if line.len() + chunk.len() > self.max_line_bytes {
                    too_long = true;
                } else {
                    line.extend_from_slice(chunk);
                }
                let used = newline.map_or(buf.len(), |i| i + 1);
                self.input.consume(used);
                if newline.is_some() {
                    break;
                }
            }
            if line.is_empty() && !too_long {
                // End of input, or a blank line.
                let at_end = self.input.fill_buf().map_err(StreamError::Io)?.is_empty();
                if at_end {
                    self.line += 1;
                    return Ok(None);
                }
                self.line += 1;
                continue;
            }
            self.line += 1;
            if too_long {
                return Err(StreamError::LineTooLong {
                    line: self.line,
                    max: self.max_line_bytes,
                });
            }
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.iter().all(u8::is_ascii_whitespace) {
                continue;
            }
            return Ok(Some(line));
        }
    }

    fn read_item(&mut self) -> Result<Option<Item>, StreamError> {
        self.header()?;
        let Some(line) = self.next_line()? else {
            if self.trailer_seen {
                return Ok(None);
            }
            return Err(StreamError::MissingTrailer);
        };
        let number = self.line;
        if self.trailer_seen {
            return Err(StreamError::AfterTrailer { line: number });
        }
        let text = std::str::from_utf8(&line).map_err(|e| StreamError::BadJson {
            line: number,
            error: e.to_string(),
        })?;
        let value: Value = serde_json::from_str(text).map_err(|e| StreamError::BadJson {
            line: number,
            error: e.to_string(),
        })?;
        if value.get("resourceType").is_some() {
            let resource = RawResource::parse(text).map_err(|e| StreamError::BadJson {
                line: number,
                error: e.message().to_string(),
            })?;
            *self
                .counts
                .entry(resource.resource_type.clone())
                .or_insert(0) += 1;
            return Ok(Some(Item::Resource(resource)));
        }
        if value.get("schema").and_then(Value::as_str) == Some(END_SCHEMA) {
            let trailer: Trailer =
                serde_json::from_value(value).map_err(|e| StreamError::BadJson {
                    line: number,
                    error: e.to_string(),
                })?;
            self.trailer_seen = true;
            if !trailer.complete {
                return Err(StreamError::Incomplete {
                    trailer: Box::new(trailer),
                });
            }
            let types: HashSet<&String> = trailer.counts.keys().chain(self.counts.keys()).collect();
            for resource_type in types {
                let said = trailer.counts.get(resource_type).copied().unwrap_or(0);
                let read = self.counts.get(resource_type).copied().unwrap_or(0);
                if said != read {
                    return Err(StreamError::CountMismatch {
                        resource_type: resource_type.clone(),
                        trailer: said,
                        read,
                    });
                }
            }
            return Ok(Some(Item::Trailer(trailer)));
        }
        Err(StreamError::NotAResource { line: number })
    }
}

impl<R: BufRead> Iterator for RecordsReader<R> {
    type Item = Result<Item, StreamError>;

    /// Each resource, then the trailer. After an error or the end, `None`.
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.read_item() {
            Ok(Some(item)) => Some(Ok(item)),
            Ok(None) => {
                self.done = true;
                None
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::relationship;

    fn header() -> Header {
        Header::new(
            Source::new("pk-test", "0.0.0"),
            Organization {
                id: "org-0001".into(),
                name: "Example Health".into(),
                fhir_base: "https://fhir.example.test/r4".into(),
                vendor: Some("epic".into()),
                location: None,
                aliases: vec![],
            },
            PatientContext {
                id: "pat-0001".into(),
                organization: "org-0001".into(),
                label: None,
                relationship: Some(relationship::SELF.into()),
                kind: None,
                member_role: None,
                fhir_user: None,
                name: None,
                session: None,
            },
            "2020-01-03T00:00:00Z",
        )
    }

    fn obs(id: &str) -> RawResource {
        RawResource::parse(&format!(
            r#"{{ "resourceType": "Observation", "id": "{id}", "valueQuantity": {{ "value": 1.50 }} }}"#
        ))
        .unwrap()
    }

    fn written(f: impl FnOnce(&mut RecordsWriter<&mut Vec<u8>>)) -> String {
        let mut buf = Vec::new();
        let mut w = RecordsWriter::start(&mut buf, &header()).unwrap();
        f(&mut w);
        w.finish("2020-01-03T00:01:00Z", None).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn a_stream_is_header_resources_trailer() {
        let text = written(|w| {
            assert!(w.resource(&obs("o1")).unwrap());
            assert!(!w.resource(&obs("o1")).unwrap());
            assert!(w.resource(&obs("o2")).unwrap());
            w.skip(Skip::search(
                "Observation?category=survey",
                skip::UNSUPPORTED,
                "HTTP 400",
            ));
        });
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4);
        assert!(lines[0].starts_with(r#"{"schema":"records-stream/v1","source":{"cli":"pk-test""#));
        assert_eq!(
            lines[1],
            r#"{"resourceType":"Observation","id":"o1","valueQuantity":{"value":1.50}}"#
        );
        let trailer: Value = serde_json::from_str(lines[3]).unwrap();
        assert_eq!(trailer["complete"], true);
        assert_eq!(trailer["counts"]["Observation"], 2);
        assert_eq!(trailer["skipped"][0]["code"], "unsupported");
        assert_eq!(trailer["skipped"][0]["resource_type"], "Observation");
        assert!(trailer.get("error").is_none());
        let header: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(header["fhir_version"], "4.0.1");
        assert!(header.get("method").is_none());
        assert!(header.get("since_unbounded").is_none());
    }

    #[test]
    fn the_reader_gives_back_what_was_written() {
        let text = written(|w| {
            w.resource(&obs("o1")).unwrap();
            w.resource(&obs("o2")).unwrap();
        });
        let mut reader = RecordsReader::new(text.as_bytes());
        assert_eq!(reader.header().unwrap(), &header());
        let items: Vec<Item> = reader.map(Result::unwrap).collect();
        assert_eq!(items.len(), 3);
        let Item::Resource(first) = &items[0] else {
            panic!()
        };
        assert_eq!(first, &obs("o1"));
        assert!(matches!(&items[2], Item::Trailer(t) if t.total() == 2));
    }

    #[test]
    fn an_oversized_resource_is_skipped_not_written() {
        let mut buf = Vec::new();
        let mut w = RecordsWriter::start(&mut buf, &header())
            .unwrap()
            .max_line_bytes(40);
        assert!(!w.resource(&obs("o1")).unwrap());
        let t = w.finish("t", None).unwrap();
        assert_eq!(t.skipped[0].code, skip::TOO_LARGE);
        assert!(t.counts.is_empty());
    }

    #[test]
    fn a_failure_after_the_header_is_an_incomplete_trailer() {
        let mut buf = Vec::new();
        let mut w = RecordsWriter::start(&mut buf, &header()).unwrap();
        w.resource(&obs("o1")).unwrap();
        let err = CliError::Upstream("the FHIR server answered HTTP 503".into());
        let t = w.finish("t", Some(&err)).unwrap();
        assert!(!t.complete);
        let e = t.error.as_ref().unwrap();
        assert_eq!((e.code.as_str(), e.exit_code), ("upstream", Some(5)));
        let mut reader = RecordsReader::new(buf.as_slice());
        assert!(matches!(reader.next(), Some(Ok(Item::Resource(_)))));
        let err = reader.next().unwrap().unwrap_err();
        assert!(matches!(err, StreamError::Incomplete { .. }), "{err}");
        assert!(err.to_string().contains("HTTP 503"), "{err}");
        assert!(reader.next().is_none());
    }

    fn read_all(text: &str) -> Result<Vec<Item>, StreamError> {
        let mut reader = RecordsReader::new(text.as_bytes());
        reader.header()?;
        reader.collect()
    }

    #[test]
    fn a_reader_refuses_a_stream_that_is_not_whole() {
        let good = written(|w| {
            w.resource(&obs("o1")).unwrap();
        });
        let lines: Vec<&str> = good.lines().collect();
        let cut = format!("{}\n{}\n", lines[0], lines[1]);
        assert!(matches!(read_all(&cut), Err(StreamError::MissingTrailer)));
        let extra = format!("{good}{}\n", lines[1]);
        assert!(matches!(
            read_all(&extra),
            Err(StreamError::AfterTrailer { line: 4 })
        ));
        let doubled = format!(
            "{}\n{}\n{}\n{}\n",
            lines[0], lines[1], r#"{"resourceType":"Observation","id":"o9"}"#, lines[2]
        );
        assert!(matches!(
            read_all(&doubled),
            Err(StreamError::CountMismatch {
                read: 2,
                trailer: 1,
                ..
            })
        ));
        let junk = format!("{}\n{{\"x\":1}}\n{}\n", lines[0], lines[2]);
        assert!(matches!(
            read_all(&junk),
            Err(StreamError::NotAResource { line: 2 })
        ));
        let bad = format!("{}\nnot json\n", lines[0]);
        assert!(matches!(
            read_all(&bad),
            Err(StreamError::BadJson { line: 2, .. })
        ));
        assert!(matches!(read_all(""), Err(StreamError::MissingHeader)));
        assert!(matches!(
            read_all(lines[1]),
            Err(StreamError::MissingHeader)
        ));
        assert!(matches!(
            read_all(r#"{"schema":"records-stream/v1"}"#),
            Err(StreamError::BadHeader(_))
        ));
        let err: CliError = StreamError::MissingTrailer.into();
        assert_eq!(err.exit_code(), 5);
    }

    #[test]
    fn crlf_and_blank_lines_are_read() {
        let good = written(|w| {
            w.resource(&obs("o1")).unwrap();
        });
        let crlf = good.replace('\n', "\r\n\r\n");
        assert_eq!(read_all(&crlf).unwrap().len(), 2);
    }

    #[test]
    fn a_line_over_the_reader_limit_is_refused() {
        let good = written(|w| {
            w.resource(&obs("o1")).unwrap();
        });
        let mut reader = RecordsReader::new(good.as_bytes()).max_line_bytes(400);
        reader.header().unwrap();
        let mut reader = reader.max_line_bytes(20);
        assert!(matches!(
            reader.next(),
            Some(Err(StreamError::LineTooLong { line: 2, max: 20 }))
        ));
    }

    #[test]
    fn a_reader_takes_a_stream_with_older_optional_fields_absent() {
        let text = concat!(
            r#"{"schema":"records-stream/v1","source":{"cli":"x","version":"0"},"organization":{"id":"o","name":"n","fhir_base":"https://h.test"},"patient":{"id":"p"},"pulled_at":"t"}"#,
            "\n",
            r#"{"schema":"records-stream-end/v1","complete":true}"#,
            "\n"
        );
        let mut reader = RecordsReader::new(text.as_bytes());
        let header = reader.header().unwrap();
        assert_eq!(header.fhir_version, "4.0.1");
        assert_eq!(header.method(), "fhir-api");
        assert!(header.patient.organization.is_empty());
        assert!(matches!(reader.next(), Some(Ok(Item::Trailer(_)))));
    }

    #[test]
    fn a_skip_from_an_error_is_coded_by_exit() {
        let s = Skip::from_error("Coverage", "c1", &CliError::Auth("refused".into()));
        assert_eq!(s.code, skip::FORBIDDEN);
        let s = Skip::from_error("Binary", "", &CliError::NotFound("gone".into()));
        assert_eq!((s.code.as_str(), s.id.as_deref()), (skip::NOT_FOUND, None));
    }
}
